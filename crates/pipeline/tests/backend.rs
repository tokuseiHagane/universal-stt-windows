use ustt_core::BackendPreference;
use ustt_pipeline::backend::{Backend, asr_model_file, detect_with, parakeet_lib};

#[test]
fn backend_maps_to_library_and_model() {
    assert_eq!(parakeet_lib(Backend::Cpu), "parakeet_cpu.dll");
    assert_eq!(parakeet_lib(Backend::Vulkan), "parakeet_vulkan.dll");
    assert_eq!(asr_model_file(Backend::Cpu), "redux-packed.gguf");
    assert_eq!(asr_model_file(Backend::Vulkan), "redux-q8_0.gguf");
}

#[test]
fn env_override_forces_cpu() {
    assert_eq!(
        detect_with(BackendPreference::Auto, Some("cpu")).selected,
        Backend::Cpu
    );
}
