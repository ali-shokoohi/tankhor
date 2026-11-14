//! Upload-format set policy. Valid masks are 1..15 with bit 1 set (Q05).

use kernel::error::{DomainError, ErrorCode};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Heic,
}

impl ImageFormat {
    #[must_use]
    pub const fn bit(self) -> u8 {
        match self {
            Self::Jpeg => 1,
            Self::Png => 2,
            Self::WebP => 4,
            Self::Heic => 8,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Png => "PNG",
            Self::WebP => "WEBP",
            Self::Heic => "HEIC",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormatSet {
    mask: u8,
}

impl FormatSet {
    #[must_use]
    pub const fn all() -> Self {
        Self { mask: 15 }
    }

    /// # Errors
    /// Rejects masks outside `1..=15` or missing JPEG.
    pub fn from_mask(mask: u8) -> Result<Self, DomainError> {
        if !(1..=15).contains(&mask) || mask & ImageFormat::Jpeg.bit() == 0 {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        Ok(Self { mask })
    }

    #[must_use]
    pub const fn mask(self) -> u8 {
        self.mask
    }

    #[must_use]
    pub const fn contains(self, format: ImageFormat) -> bool {
        self.mask & format.bit() != 0
    }

    #[must_use]
    pub fn formats(self) -> Vec<ImageFormat> {
        [
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::WebP,
            ImageFormat::Heic,
        ]
        .into_iter()
        .filter(|format| self.contains(*format))
        .collect()
    }
}

impl FromStr for FormatSet {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut mask = 0;
        for item in value.split(',') {
            let format = match item.trim() {
                "JPEG" => ImageFormat::Jpeg,
                "PNG" => ImageFormat::Png,
                "WEBP" => ImageFormat::WebP,
                "HEIC" => ImageFormat::Heic,
                _ => return Err(DomainError::validation(ErrorCode::InvalidValue)),
            };
            let updated = mask | format.bit();
            if updated == mask {
                return Err(DomainError::validation(ErrorCode::InvalidValue));
            }
            mask = updated;
        }
        Self::from_mask(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::{FormatSet, ImageFormat};
    use crate::domain::settings::{SettingKey, initial_upload_formats};
    use std::str::FromStr;

    #[test]
    fn upload_format_policy_requires_jpeg_and_known_formats() {
        for mask in 0u8..=31 {
            let result = FormatSet::from_mask(mask);
            assert_eq!(result.is_ok(), (1..=15).contains(&mask) && mask & 1 == 1);
        }
        assert!(FormatSet::from_mask(16).is_err());
        assert!(FormatSet::from_mask(u8::MAX).is_err());

        let all = FormatSet::from_mask(15).unwrap();
        assert_eq!(
            all.formats(),
            vec![
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::WebP,
                ImageFormat::Heic,
            ]
        );
        assert_eq!(all.mask(), 15);
        assert!(all.contains(ImageFormat::Jpeg));
        assert_eq!(FormatSet::from_str("HEIC,JPEG,WEBP,PNG").unwrap(), all);
        assert!(FormatSet::from_str("PNG,WEBP").is_err());
        assert!(FormatSet::from_str("JPEG,GIF").is_err());

        let setting = initial_upload_formats();
        assert_eq!(setting.key(), SettingKey::UploadFormats);
        assert_eq!(setting.value(), all);
        assert_eq!(setting.fa_label_key(), "setting-upload_formats-label");
    }
}
