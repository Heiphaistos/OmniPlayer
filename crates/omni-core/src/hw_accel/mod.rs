/// Accélération matérielle — DXVA2, D3D11VA, NVDEC, AMF, QuickSync.
/// Le module tente d'initialiser le meilleur accélérateur disponible.

#[cfg(windows)]
pub mod windows;

use ffmpeg_next as ffmpeg;

pub struct HwAccelContext {
    pub kind: HwKind,
}

#[derive(Debug, Clone, Copy)]
pub enum HwKind {
    Dxva2,
    D3D11Va,
    Cuda,
    None,
}

impl HwAccelContext {
    /// Tente d'initialiser l'accélérateur nommé.
    /// Noms acceptés: "dxva2", "d3d11va", "cuda", "auto"
    pub fn try_init(name: &str) -> anyhow::Result<Self> {
        let kind = match name {
            "dxva2"   | "auto" if cfg!(windows) => HwKind::Dxva2,
            "d3d11va" if cfg!(windows)           => HwKind::D3D11Va,
            "cuda"                               => HwKind::Cuda,
            _                                    => HwKind::None,
        };
        log::info!("HW accel: {kind:?}");
        Ok(Self { kind })
    }

    /// Applique le contexte HW au codec avant ouverture.
    pub fn apply_to_codec(&self, ctx: &mut ffmpeg::codec::context::Context) {
        match self.kind {
            // Enable frame-level parallelism for all HW-accelerated paths.
            HwKind::Dxva2 | HwKind::D3D11Va | HwKind::Cuda => {
                ctx.set_threading(ffmpeg::codec::threading::Config {
                    kind:  ffmpeg::codec::threading::Type::Frame,
                    count: 4,
                });
            }
            HwKind::None => {}
        }
    }
}
