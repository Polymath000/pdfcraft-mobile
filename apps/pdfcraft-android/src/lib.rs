#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#![allow(unsafe_code)]


#[cfg(target_os = "android")]

use winit::platform::android::activity::AndroidApp;


#[cfg(target_os = "android")]

#[unsafe(no_mangle)]

fn android_main(app: AndroidApp) {

    android_logger::init_once(

        android_logger::Config::default()

            .with_max_level(log::LevelFilter::Debug)

            .with_tag("PdfCraft"),

    );


    log::info!("Starting PdfCraft Android application...");


    let options = eframe::NativeOptions {

        android_app: Some(app),

        renderer: eframe::Renderer::Glow,

        ..Default::default()

    };


    let result = eframe::run_native(

        "PdfCraft",

        options,

        Box::new(|cc| {

            log::info!("PdfCraft app creator called");

            pdfcraft_ui_egui::layout_traits::init_android_safe_area(&cc.egui_ctx);

            let mut app = pdfcraft_ui_egui::PdfCraftApp::new();

            if let Some(json) = cc.storage.and_then(|s| s.get_string("pdfcraft")) {

                app.restore(&json);

            }

            Ok(Box::new(app))

        }),

    );


    match result {

        Ok(()) => log::info!("eframe::run_native exited cleanly"),

        Err(e) => log::error!("eframe::run_native error: {e:?}"),

    }

}

