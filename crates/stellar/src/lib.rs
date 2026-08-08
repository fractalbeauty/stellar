pub mod core;
pub mod error;

pub use stellar_graph as graph;
pub use stellar_log as log;
pub use stellar_sync as sync;

uniffi::setup_scaffolding!();

// On Android, expose a JNI function to initialize `ndk_context`
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_trillia_stellar_RustNdkContext_init(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
    context: jni::objects::JObject,
) {
    let java_vm = env.get_java_vm().expect("failed to get java vm");
    let java_vm_ptr = java_vm.get_java_vm_pointer() as *mut std::ffi::c_void;

    // Turn the local context reference into a global reference
    let context = env
        .new_global_ref(context)
        .expect("failed to create global ref for context");
    let context_ptr = context.as_raw() as *mut std::ffi::c_void;

    // Leak the context global reference so it stays alive forever
    Box::leak(Box::new(context));

    unsafe {
        ndk_context::initialize_android_context(java_vm_ptr, context_ptr);
    }
}

#[uniffi::export]
pub fn log_trace(message: String) {
    tracing::trace!("compose: {}", message);
}

#[uniffi::export]
pub fn log_debug(message: String) {
    tracing::debug!("compose: {}", message);
}

#[uniffi::export]
pub fn log_info(message: String) {
    tracing::info!("compose: {}", message);
}

#[uniffi::export]
pub fn log_warn(message: String) {
    tracing::warn!("compose: {}", message);
}

#[uniffi::export]
pub fn log_error(message: String) {
    tracing::error!("compose: {}", message);
}
