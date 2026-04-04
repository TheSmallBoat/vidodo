//! Integration tests for visual-bridge crate.

#[cfg(test)]
mod integration {
    use crate::device::{DeviceConfig, DeviceManager, DeviceState, GpuBackend};
    use crate::shader::{CompileResult, ShaderCompiler, ShaderSource, ShaderStage};
    use crate::window::{DisplayEndpoint, WindowConfig, WindowManager};

    #[test]
    fn device_init_and_window_create_flow() {
        let mut device = DeviceManager::new(DeviceConfig::default());
        device.initialize().unwrap();
        assert_eq!(*device.state(), DeviceState::Ready);

        let mut windows = WindowManager::new();
        let idx = windows
            .create_window(DisplayEndpoint {
                display_id: "main".into(),
                os_handle: None,
                window: WindowConfig::default(),
                role: "main".into(),
            })
            .unwrap();

        windows.present_frame(idx).unwrap();
        assert_eq!(windows.windows()[idx].frame_count, 1);
    }

    #[test]
    fn multi_window_with_shader_compile() {
        let mut windows = WindowManager::new();
        for name in &["left", "center", "right"] {
            windows
                .create_window(DisplayEndpoint {
                    display_id: name.to_string(),
                    os_handle: None,
                    window: WindowConfig {
                        title: format!("Vidodo {name}"),
                        ..WindowConfig::default()
                    },
                    role: name.to_string(),
                })
                .unwrap();
        }
        assert_eq!(windows.open_count(), 3);

        let mut compiler = ShaderCompiler::new();
        let result = compiler.compile(&ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: "#version 450\nvoid main() { gl_Position = vec4(0); }".into(),
            entry_point: "main".into(),
            label: "test.vert".into(),
        });
        assert!(matches!(result, CompileResult::Ok { .. }));
    }

    #[test]
    fn device_loss_recovery_with_windows() {
        let mut device = DeviceManager::new(DeviceConfig {
            backend: GpuBackend::Metal,
            ..DeviceConfig::default()
        });
        device.initialize().unwrap();
        let info = device.info().unwrap();
        assert_eq!(info.backend, GpuBackend::Metal);

        // Simulate device loss
        device.mark_lost();
        assert_eq!(*device.state(), DeviceState::Lost);

        // Recover
        device.recover().unwrap();
        assert_eq!(*device.state(), DeviceState::Ready);
    }
}
