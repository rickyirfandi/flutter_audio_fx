//! Android JNI glue.
//!
//! cpal's Android (oboe) backend reads the JavaVM and Android `Context` from
//! `ndk_context` when it queries supported stream configs / min buffer sizes.
//! A `dart:ffi` `dlopen` does NOT trigger `JNI_OnLoad`, and Flutter does not
//! wire `ndk_context` for FFI plugins — so without this, any cpal device call
//! panics (the backend `.unwrap()`s the JNI calls) and aborts the app.
//!
//! Flow: the Kotlin `FlutterAudioFxPlugin` `System.loadLibrary`s this core
//! (firing `JNI_OnLoad`, which captures the `JavaVM`) and then calls
//! `nativeAttachContext(applicationContext)`, which publishes both into
//! `ndk_context` before any audio call runs.

use std::ffi::c_void;
use std::sync::OnceLock;

use jni::objects::{JClass, JObject};
use jni::sys::{jint, JNI_VERSION_1_6};
use jni::{JNIEnv, JavaVM};

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

/// Called by the Android runtime when the shared library is loaded via
/// `System.loadLibrary`. Captures the process-wide `JavaVM`.
#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    if let Ok(vm) = unsafe { JavaVM::from_raw(vm) } {
        let _ = JAVA_VM.set(vm);
    }
    JNI_VERSION_1_6
}

/// Publishes the `JavaVM` + application `Context` into `ndk_context` so cpal's
/// oboe backend can attach to the JVM. Safe to call more than once.
///
/// JNI symbol for Kotlin class `dev.flutter_audio_fx.FlutterAudioFxPlugin`.
#[no_mangle]
pub extern "system" fn Java_dev_flutter_1audio_1fx_FlutterAudioFxPlugin_nativeAttachContext(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    let Some(vm) = JAVA_VM.get() else {
        log::error!("nativeAttachContext: JavaVM not captured by JNI_OnLoad");
        return;
    };
    let global = match env.new_global_ref(&context) {
        Ok(g) => g,
        Err(e) => {
            log::error!("nativeAttachContext: new_global_ref failed: {e}");
            return;
        }
    };
    unsafe {
        ndk_context::initialize_android_context(
            vm.get_java_vm_pointer() as *mut c_void,
            global.as_raw() as *mut c_void,
        );
    }
    // The Context must live for the whole process — cpal reads it on every
    // device/config query. Leak the global ref so it is never deleted.
    std::mem::forget(global);
}
