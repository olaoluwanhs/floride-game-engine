#pragma once

// Portable React Native JSI HostObject template for Floride game cores.
//
// Usage inside a React Native C++ TurboModule / JSI module:
//   1. Include your game's cbindgen header (e.g. "sof_host.h").
//   2. Supply a `FlorideCAbiVTable` populated with your game's C-ABI symbols.
//   3. Install an instance into `facebook::jsi::Runtime` via:
//        jsi::Object::createFromHostObject(rt, std::make_shared<FlorideJsiHostObject>(...))
//
// Thread-ownership contract:
//   `FlorideSessionHandle*` is NOT internally mutex-locked. Either call all methods
//   on the JS thread (matching React Native's synchronous JSI loop) OR serialize all
//   create/dispatch/tick/copy_snapshot/destroy calls under a dedicated simulation mutex
//   on a background thread and expose a copied buffer to JS.

#include <cstdint>
#include <cstring>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

#if __has_include(<jsi/jsi.h>)
#include <jsi/jsi.h>

namespace floride {

struct FlorideSessionHandle;

struct FlorideCAbiVTable {
    uint32_t (*abi_version)();
    uint16_t (*protocol_version)();
    FlorideSessionHandle* (*create_default)();
    FlorideSessionHandle* (*create_from_json)(const uint8_t* json_ptr, size_t json_len);
    void (*destroy)(FlorideSessionHandle* handle);
    int32_t (*dispatch_action)(FlorideSessionHandle* handle, uint16_t action_id, const uint8_t* payload_ptr, size_t payload_len);
    int32_t (*tick)(FlorideSessionHandle* handle);
    int32_t (*copy_snapshot)(const FlorideSessionHandle* handle, uint8_t* out_ptr, size_t capacity, size_t* out_written);
    size_t (*snapshot_len)(const FlorideSessionHandle* handle);
    size_t (*last_error_len)();
    int32_t (*copy_last_error)(uint8_t* out_ptr, size_t capacity, size_t* out_written);
};

class FlorideSnapshotBuffer final : public facebook::jsi::MutableBuffer {
public:
    explicit FlorideSnapshotBuffer(std::vector<uint8_t> bytes) : bytes_(std::move(bytes)) {}

    size_t size() const override { return bytes_.size(); }
    uint8_t* data() override { return bytes_.data(); }

private:
    std::vector<uint8_t> bytes_;
};

class FlorideJsiHostObject final : public facebook::jsi::HostObject {
public:
    FlorideJsiHostObject(FlorideCAbiVTable api, FlorideSessionHandle* handle)
        : api_(api), handle_(handle) {
        if (handle_ == nullptr) {
            throw std::runtime_error("FlorideJsiHostObject requires a non-null session handle");
        }
    }

    ~FlorideJsiHostObject() override {
        std::lock_guard<std::mutex> lock(mu_);
        if (handle_ != nullptr && api_.destroy != nullptr) {
            api_.destroy(handle_);
            handle_ = nullptr;
        }
    }

    FlorideJsiHostObject(const FlorideJsiHostObject&) = delete;
    FlorideJsiHostObject& operator=(const FlorideJsiHostObject&) = delete;

    facebook::jsi::Value get(facebook::jsi::Runtime& rt, const facebook::jsi::PropNameID& name) override {
        const std::string key = name.utf8(rt);

        if (key == "dispatchAction") {
            return facebook::jsi::Function::createFromHostFunction(
                rt,
                name,
                2,
                [this](facebook::jsi::Runtime& runtime,
                       const facebook::jsi::Value&,
                       const facebook::jsi::Value* args,
                       size_t count) -> facebook::jsi::Value {
                    if (count < 1 || !args[0].isNumber()) {
                        throw facebook::jsi::JSError(runtime, "dispatchAction(actionId, payload?) requires numeric actionId");
                    }
                    const auto action_id = static_cast<uint16_t>(args[0].asNumber());
                    const uint8_t* ptr = nullptr;
                    size_t len = 0;

                    if (count >= 2 && !args[1].isUndefined() && !args[1].isNull()) {
                        auto obj = args[1].asObject(runtime);
                        if (obj.isArrayBuffer(runtime)) {
                            auto buf = obj.getArrayBuffer(runtime);
                            ptr = buf.data(runtime);
                            len = buf.size(runtime);
                        } else if (obj.hasProperty(runtime, "buffer")) {
                            auto buffer_prop = obj.getProperty(runtime, "buffer").asObject(runtime);
                            auto buf = buffer_prop.getArrayBuffer(runtime);
                            const size_t byte_offset = obj.hasProperty(runtime, "byteOffset")
                                ? static_cast<size_t>(obj.getProperty(runtime, "byteOffset").asNumber())
                                : 0;
                            const size_t byte_length = obj.hasProperty(runtime, "byteLength")
                                ? static_cast<size_t>(obj.getProperty(runtime, "byteLength").asNumber())
                                : buf.size(runtime);
                            ptr = buf.data(runtime) + byte_offset;
                            len = byte_length;
                        } else {
                            throw facebook::jsi::JSError(runtime, "payload must be an ArrayBuffer or TypedArray");
                        }
                    }

                    std::lock_guard<std::mutex> lock(mu_);
                    ensure_live(runtime);
                    const int32_t rc = api_.dispatch_action(handle_, action_id, ptr, len);
                    if (rc != 0) {
                        throw facebook::jsi::JSError(runtime, read_last_error("dispatchAction failed"));
                    }
                    return facebook::jsi::Value::undefined();
                });
        }

        if (key == "tick") {
            return facebook::jsi::Function::createFromHostFunction(
                rt,
                name,
                0,
                [this](facebook::jsi::Runtime& runtime,
                       const facebook::jsi::Value&,
                       const facebook::jsi::Value*,
                       size_t) -> facebook::jsi::Value {
                    std::lock_guard<std::mutex> lock(mu_);
                    ensure_live(runtime);
                    const int32_t rc = api_.tick(handle_);
                    if (rc != 0) {
                        throw facebook::jsi::JSError(runtime, read_last_error("tick failed"));
                    }
                    return facebook::jsi::Value::undefined();
                });
        }

        if (key == "readSnapshot") {
            return facebook::jsi::Function::createFromHostFunction(
                rt,
                name,
                0,
                [this](facebook::jsi::Runtime& runtime,
                       const facebook::jsi::Value&,
                       const facebook::jsi::Value*,
                       size_t) -> facebook::jsi::Value {
                    std::vector<uint8_t> bytes;
                    {
                        std::lock_guard<std::mutex> lock(mu_);
                        ensure_live(runtime);
                        const size_t needed = api_.snapshot_len(handle_);
                        bytes.resize(needed);
                        size_t written = 0;
                        const int32_t rc = api_.copy_snapshot(handle_, bytes.data(), bytes.size(), &written);
                        if (rc != 0) {
                            throw facebook::jsi::JSError(runtime, read_last_error("copy_snapshot failed"));
                        }
                        bytes.resize(written);
                    }
                    auto buffer = std::make_shared<FlorideSnapshotBuffer>(std::move(bytes));
                    return facebook::jsi::ArrayBuffer(runtime, buffer);
                });
        }

        if (key == "destroy") {
            return facebook::jsi::Function::createFromHostFunction(
                rt,
                name,
                0,
                [this](facebook::jsi::Runtime&,
                       const facebook::jsi::Value&,
                       const facebook::jsi::Value*,
                       size_t) -> facebook::jsi::Value {
                    std::lock_guard<std::mutex> lock(mu_);
                    if (handle_ != nullptr && api_.destroy != nullptr) {
                        api_.destroy(handle_);
                        handle_ = nullptr;
                    }
                    return facebook::jsi::Value::undefined();
                });
        }

        return facebook::jsi::Value::undefined();
    }

private:
    void ensure_live(facebook::jsi::Runtime& rt) const {
        if (handle_ == nullptr) {
            throw facebook::jsi::JSError(rt, "Floride session has already been destroyed");
        }
    }

    std::string read_last_error(const char* fallback) const {
        if (api_.last_error_len == nullptr || api_.copy_last_error == nullptr) {
            return fallback;
        }
        const size_t len = api_.last_error_len();
        if (len == 0) {
            return fallback;
        }
        std::string msg(len, '\0');
        size_t written = 0;
        if (api_.copy_last_error(reinterpret_cast<uint8_t*>(&msg[0]), len, &written) == 0) {
            msg.resize(written);
            return msg;
        }
        return fallback;
    }

    FlorideCAbiVTable api_;
    FlorideSessionHandle* handle_;
    std::mutex mu_;
};

} // namespace floride

#endif // __has_include(<jsi/jsi.h>)
