pub mod audio;
pub mod context;
pub mod subtitle;
pub mod video;

pub use audio::DecodedAudioFrame;
pub use video::DecodedVideoFrame;

/// Format pixel brut livré au renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Yuv420p,     // planar YUV 4:2:0 8-bit — le plus courant
    Yuv422p,     // planar YUV 4:2:2
    Yuv444p,     // planar YUV 4:4:4
    Nv12,        // semi-planar NV12 (HW accel output)
    /// Planar YUV 4:2:0 10-bit (HDR10/HLG). Échantillons 16-bit, décalés de 6
    /// bits vers la gauche (convention P010) : la valeur 10-bit d'origine
    /// occupe les bits hauts, ce qui permet au shader existant (pensé pour
    /// des textures normalisées 8-bit) de fonctionner sans changement — le
    /// ratio noir/blanc/plage limitée est identique en 8 et 10 bits.
    Yuv420p10le,
    #[allow(dead_code)] P010Le, // réservé : semi-planaire 10-bit (non produit actuellement)
    Rgba,        // fallback RGBA
}

impl PixelFormat {
    pub fn is_hdr10bit(self) -> bool {
        matches!(self, PixelFormat::Yuv420p10le)
    }
}
