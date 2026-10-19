//! Control API — lets a Stream Deck key send a command to the signed-in
//! user's app and hear back whether it worked. Same addresses and answers as
//! the web version, so the existing Stream Deck plugin keeps working:
//!
//!   GET/POST http://localhost:47914/api/<app>/<command>[/<arg>]
//!
//! It only listens on this PC, so no key is needed: commands go to whoever is
//! signed in. Requests from web pages are refused (browsers label them with the
//! site they came from), so a website you visit can't press the keys for you.

use crate::core::{lock, ServerStatus, Shared, UiEvent};
use serde_json::{json, Map, Value};
use std::io::Cursor;

/// Melbourne Fire Brigade's own port (the Stream Deck plugin uses the same one). Chosen so that
/// nothing else uses it: not a well-known port, and below the range Windows
/// hands out to other programs at random.
pub const PORT: u16 = 47914;
const APPS: [(&str, &[&str]); 2] = [
    ("fieldguide", &["open", "home", "next", "prev", "option", "mica", "step", "unstep", "reset", "reset-all"]),
    ("timeclock", &["toggle", "in", "out", "status"]),
];

pub fn start(core: Shared, ctx: eframe::egui::Context) {
    // Runs in the background: right after an update the old copy may still be
    // letting go of the port, so keep trying for up to ten seconds.
    std::thread::spawn(move || {
        let serve = |server: tiny_http::Server, core: Shared, ctx: eframe::egui::Context| {
            std::thread::spawn(move || {
                for request in server.incoming_requests() {
                    let core = core.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || handle(request, &core, &ctx));
                }
            });
        };
        // "localhost" can mean IPv4 or IPv6, so listen on both (IPv6 is a bonus).
        let mut v6 = false;
        let mut error = String::new();
        for _attempt in 0..40 {
            if !v6 {
                if let Ok(server) = tiny_http::Server::http(format!("[::1]:{PORT}")) {
                    serve(server, core.clone(), ctx.clone());
                    v6 = true;
                }
            }
            match tiny_http::Server::http(format!("127.0.0.1:{PORT}")) {
                Ok(server) => {
                    serve(server, core.clone(), ctx.clone());
                    lock(&core).server = ServerStatus::Running;
                    ctx.request_repaint();
                    return;
                }
                Err(e) => error = e.to_string(),
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        lock(&core).server = ServerStatus::Failed(format!("Port {PORT} is busy or blocked ({error}). Is another copy of Melbourne Fire Brigade running?"));
        ctx.request_repaint();
    });
}

/// A header's value, if the request has it.
fn header(request: &tiny_http::Request, name: &'static str) -> Option<String> {
    request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string())
}

/// True when a web browser sent this (a page you have open, not the Stream Deck).
/// Browsers always say which site a request came from; the Stream Deck plugin doesn't.
fn from_browser(request: &tiny_http::Request) -> bool {
    let site = header(request, "sec-fetch-site").map(|s| s.to_ascii_lowercase());
    header(request, "origin").is_some() || site.is_some_and(|s| s != "none")
}

/// Only addresses that mean "this PC" (stops a website renaming itself to reach us).
fn local_host(request: &tiny_http::Request) -> bool {
    let Some(host) = header(request, "host") else { return true };
    let host = host.to_ascii_lowercase();
    let name = host.rsplit_once(':').map_or(host.as_str(), |(h, _)| h);
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
}

fn route(path: &str) -> Option<(String, String, Option<String>)> {
    let rest = path.strip_prefix("/api/")?.trim_end_matches('/');
    let parts: Vec<&str> = rest.split('/').collect();
    let ok = |s: &str, extra: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || extra.contains(c) || (extra.contains('0') && c.is_ascii_digit()));
    match parts.as_slice() {
        [app, cmd] if ok(app, "") && ok(cmd, "-") => Some((app.to_string(), cmd.to_string(), None)),
        [app, cmd, arg] if ok(app, "") && ok(cmd, "-") && ok(arg, "-0") => Some((app.to_string(), cmd.to_string(), Some(arg.to_string()))),
        _ => None,
    }
}

fn handle(request: tiny_http::Request, core: &Shared, ctx: &eframe::egui::Context) {
    let (status, body) = respond(&request, core, ctx);
    let mut response = tiny_http::Response::new(
        tiny_http::StatusCode(status),
        vec![],
        Cursor::new(body.to_string().into_bytes()),
        None,
        None,
    );
    for (k, v) in [("Content-Type", "application/json"), ("Cache-Control", "no-store")] {
        response.add_header(tiny_http::Header::from_bytes(k, v).unwrap());
    }
    let _ = request.respond(response);
}

fn respond(request: &tiny_http::Request, core: &Shared, ctx: &eframe::egui::Context) -> (u16, Value) {
    let url = request.url().to_string();
    let path = url.split_once('?').map_or(url.as_str(), |(p, _)| p);
    if !path.starts_with("/api/") {
        return (404, json!({ "ok": false, "error": "Not found" }));
    }
    if from_browser(request) || !local_host(request) {
        return (403, json!({ "ok": false, "error": "Web pages can't control Melbourne Fire Brigade" }));
    }
    let method = request.method().as_str().to_ascii_uppercase();
    if method != "GET" && method != "POST" {
        return (405, json!({ "ok": false, "error": "Use GET or POST" }));
    }
    let Some((app, command, arg)) = route(path) else { return (404, json!({ "ok": false, "error": "Unknown command" })) };
    if !APPS.iter().any(|(a, cmds)| *a == app && cmds.contains(&command.as_str())) {
        return (404, json!({ "ok": false, "error": "Unknown command" }));
    }

    let mut c = lock(core);
    if c.user.is_none() {
        return (409, json!({ "ok": false, "error": "Nobody is signed in right now" }));
    }
    let name = c.user.clone().unwrap_or_default();
    let mut sent = Map::new();
    sent.insert("app".into(), json!(app));
    sent.insert("command".into(), json!(command));
    sent.insert("arg".into(), json!(arg));

    let (ok, data): (bool, Map<String, Value>) = if app == "fieldguide" {
        let ok = c.guide.run(&command, arg.as_deref());
        c.events.push(UiEvent::OpenApp("fieldguide"));
        let mut data = Map::new();
        if !ok {
            data.insert("error".into(), json!("Nothing to do for that on this page"));
        }
        (ok, data)
    } else {
        let changed = match command.as_str() {
            "toggle" => c.sheet.toggle(&name),
            "in" => c.sheet.clock_in(&name),
            "out" => c.sheet.clock_out(&name),
            _ => false,
        };
        if command != "status" && changed {
            let at = crate::ui::format::time(chrono::Local::now());
            let text = if c.sheet.on_duty() { format!("Clocked in at {at}") } else { format!("Clocked out at {at}") };
            c.events.push(UiEvent::Toast(text));
        }
        // Clocking in when already in (or out when out) isn't an error: the key just shows the real state.
        let mut data = Map::new();
        data.insert("onDuty".into(), json!(c.sheet.on_duty()));
        data.insert("since".into(), json!(c.sheet.active_since));
        (true, data)
    };
    drop(c);
    ctx.request_repaint();

    let mut body = Map::new();
    body.insert("ok".into(), json!(ok));
    body.extend(sent);
    if ok {
        body.extend(data);
        (200, Value::Object(body))
    } else {
        body.insert("error".into(), data.get("error").cloned().unwrap_or(json!("The app could not do that right now")));
        (422, Value::Object(body))
    }
}
