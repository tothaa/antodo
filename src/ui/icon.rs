//! Window identity must match assets/antodo.desktop for Wayland taskbars.
use iced::window;

pub(super) fn settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(1320.0, 900.0),
        min_size: Some(iced::Size::new(880.0, 620.0)),
        exit_on_close_request: false,
        // Converted from antodo.svg with rsvg-convert at 128x128, then
        // `magick antodo.png -depth 8 rgba:antodo.rgba`. No runtime file lookup.
        icon: window::icon::from_rgba(
            include_bytes!("../../assets/antodo.rgba").to_vec(),
            128,
            128,
        )
        .ok(),
        platform_specific: {
            #[cfg(target_os = "linux")]
            {
                window::settings::PlatformSpecific {
                    application_id: "antodo".into(),
                    ..Default::default()
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                Default::default()
            }
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_uses_bundled_icon_and_matching_desktop_identity() {
        let settings = super::settings();
        let (pixels, size) = settings.icon.unwrap().into_raw();
        assert_eq!((size.width, size.height), (128, 128));
        assert_eq!(pixels.len(), 128 * 128 * 4);
        assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
        #[cfg(target_os = "linux")]
        assert_eq!(settings.platform_specific.application_id, "antodo");
    }
}
