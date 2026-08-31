use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built frontend, baked into the binary.
///
/// In development this is empty and unused: Vite serves the app on :5173
/// and proxies /api and /ws here. `web/dist/.gitkeep` keeps the directory
/// present so a clean checkout compiles.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

const PLACEHOLDER: &str = "<!doctype html><meta charset=utf-8>\
<title>grubsi</title>\
<body style=\"font-family:system-ui;max-width:34rem;margin:4rem auto;line-height:1.6\">\
<h1>Frontend not built</h1>\
<p>The API is running. To build the interface:</p>\
<pre>npm --prefix web ci &amp;&amp; npm --prefix web run build</pre>\
<p>For the development loop, run <code>just web</code> and open \
<a href=\"http://localhost:5173\">localhost:5173</a> instead.</p>";

pub async fn serve_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response();
    }

    // Anything else is a client-side route: hand back the shell.
    match Assets::get("index.html") {
        Some(index) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            index.data,
        )
            .into_response(),
        None => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            PLACEHOLDER,
        )
            .into_response(),
    }
}
