use std::path::Path;

use zsm_core::store::messages::AttachmentRecord;
use zsm_mime::{block_remote_images, rewrite_cid_images};

use crate::core::Core;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    pub status: u16,
    pub content_type: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Served {
    fn text(status: u16, message: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            headers: base_headers("default-src 'none'"),
            body: message.as_bytes().to_vec(),
        }
    }
}

fn base_headers(csp: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Content-Security-Policy", csp.to_string()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Referrer-Policy", "no-referrer".into()),
        ("Cross-Origin-Resource-Policy", "cross-origin".into()),
    ]
}

fn flag(query: Option<&str>, name: &str) -> bool {
    query.is_some_and(|q| q.split('&').any(|pair| pair == format!("{name}=1") || pair == name))
}

fn previewable(mime: &str) -> bool {
    matches!(
        mime,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/avif"
            | "image/svg+xml"
            | "application/pdf"
            | "text/plain"
            | "audio/mpeg"
            | "audio/ogg"
            | "audio/wav"
            | "video/mp4"
            | "video/webm"
    )
}

impl Core {
    pub fn serve(&self, path: &str, query: Option<&str>) -> Served {
        let path = path.trim_start_matches('/');
        let (kind, id) = path.split_once('/').unwrap_or((path, ""));
        let Ok(id) = id.parse::<i64>() else {
            return Served::text(404, "Not found");
        };
        match kind {
            "msg" => self.serve_message(id, flag(query, "remote")),
            "cid" => self.serve_attachment(id, true),
            "att" => self.serve_attachment(id, flag(query, "inline")),
            _ => Served::text(404, "Not found"),
        }
    }

    fn serve_message(&self, id: i64, allow_remote: bool) -> Served {
        let found = self.with_store(|s| {
            let Some(html) = s.message_html(id)? else {
                return Ok(None);
            };
            let html = rewrite_cid_images(&html, |cid| {
                s.attachment_by_content_id(id, cid).ok().flatten().map(|a| format!("../cid/{}", a.id))
            });
            Ok(Some(html))
        });
        let Ok(Some(html)) = found else {
            return Served::text(404, "This message has no formatted version");
        };
        let html = if allow_remote { html } else { block_remote_images(&html) };
        let nonce = format!("{:032x}", rand::random::<u128>());
        let csp = message_csp(&nonce, allow_remote);
        let document = MESSAGE_DOCUMENT
            .replace("{{CSP}}", &html_escape::encode_double_quoted_attribute(&csp))
            .replace("{{NONCE}}", &nonce)
            .replace("{{ID}}", &id.to_string())
            .replace("{{BODY}}", &html);
        let mut headers = base_headers(&csp);
        headers.push(("Cache-Control", "no-store".into()));
        Served { status: 200, content_type: "text/html; charset=utf-8".into(), headers, body: document.into_bytes() }
    }

    fn serve_attachment(&self, id: i64, inline_only: bool) -> Served {
        let record = match self.with_store(|s| Ok(s.attachment(id)?)) {
            Ok(Some(record)) => record,
            _ => return Served::text(404, "That file is not available"),
        };
        let Some((mime, bytes)) = self.attachment_bytes(&record) else {
            return Served::text(404, "That file has not been downloaded yet");
        };
        let show = (inline_only || record.inline) && previewable(&mime);
        let mut headers = base_headers("sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:");
        headers.push(("Cache-Control", "private, max-age=3600".into()));
        let disposition = if show { "inline" } else { "attachment" };
        headers.push((
            "Content-Disposition",
            format!("{disposition}; filename*=UTF-8''{}", percent_encode(&record.filename)),
        ));
        let content_type = if show { mime } else { "application/octet-stream".into() };
        Served { status: 200, content_type, headers, body: bytes }
    }

    pub(crate) fn attachment_bytes(&self, record: &AttachmentRecord) -> Option<(String, Vec<u8>)> {
        if let Some(path) = record.cache_path.as_deref().map(Path::new)
            && let Ok(bytes) = std::fs::read(path)
        {
            return Some((record.mime.clone(), bytes));
        }
        let demo = self
            .with_store(|s| Ok(s.accounts()?.iter().any(|a| a.id == record.account_id && a.provider == "demo")))
            .unwrap_or(false);
        demo.then(|| demo_file(&record.filename, &record.mime))
    }
}

impl Core {
    pub fn save_attachment(&self, id: i64) -> crate::error::Result<String> {
        use crate::error::Error;
        let record =
            self.with_store(|s| Ok(s.attachment(id)?))?.ok_or(Error::Store(zsm_core::Error::NotFound("file")))?;
        let (_, bytes) = self
            .attachment_bytes(&record)
            .ok_or_else(|| Error::Store(zsm_core::Error::Invalid("That file has not been downloaded yet".into())))?;
        let dir = self
            .downloads_dir
            .as_ref()
            .ok_or_else(|| Error::Store(zsm_core::Error::Invalid("There is no Downloads folder to save to".into())))?;
        std::fs::create_dir_all(dir)?;
        let path = unique_path(dir, &safe_file_name(&record.filename));
        std::fs::write(&path, bytes)?;
        Ok(path.display().to_string())
    }
}

fn safe_file_name(name: &str) -> String {
    let cleaned: String =
        name.chars()
            .map(|c| {
                if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '_'
                } else {
                    c
                }
            })
            .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim_end_matches(['.', ' ']).trim();
    if trimmed.is_empty() {
        return "attachment".to_string();
    }
    let mut name: String = trimmed.chars().take(180).collect();
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            stem.strip_prefix(p).is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        });
    if reserved {
        name.insert(0, '_');
    }
    name
}

fn unique_path(dir: &Path, name: &str) -> std::path::PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).expect("an unused name exists")
}

fn message_csp(nonce: &str, allow_remote: bool) -> String {
    let images = if allow_remote { "img-src 'self' data: https:" } else { "img-src 'self' data:" };
    format!(
        "default-src 'none'; script-src 'nonce-{nonce}'; style-src 'unsafe-inline'; {images}; font-src 'none'; \
         base-uri 'none'; form-action 'none'; frame-src 'none'; object-src 'none'"
    )
}

const MESSAGE_DOCUMENT: &str = r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="{{CSP}}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light">
<style>
html{background:#fdfcf9;color:#172135}
body{margin:0;padding:2px 2px 6px;font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;overflow-wrap:anywhere;word-break:normal}
img{max-width:100%;height:auto}
table{max-width:100%}
a{color:#005798}
blockquote{margin:0 0 0 .6em;padding-left:.8em;border-left:2px solid #d1d8df;color:#4d5969}
pre{white-space:pre-wrap}
img[data-zsm-src]{display:inline-block;min-width:28px;min-height:20px;background:#e6eef4;outline:1px dashed #9aa6b2;outline-offset:-1px}
</style></head>
<body>{{BODY}}
<script nonce="{{NONCE}}">
(function(){
  var id={{ID}};
  function post(m){parent.postMessage(m,'*');}
  function size(){post({zsm:'frame',id:id,height:Math.ceil(document.documentElement.getBoundingClientRect().height)});}
  new ResizeObserver(size).observe(document.documentElement);
  addEventListener('load',size);
  document.addEventListener('click',function(e){
    var a=e.target&&e.target.closest?e.target.closest('a[href]'):null;
    if(!a)return;
    e.preventDefault();
    post({zsm:'link',id:id,href:a.href,text:(a.textContent||'').trim().slice(0,300)});
  },true);
  size();
})();
</script></body></html>"#;

fn percent_encode(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn demo_file(filename: &str, mime: &str) -> (String, Vec<u8>) {
    let lower = filename.to_lowercase();
    if mime.starts_with("image/") {
        return ("image/svg+xml".into(), demo_image(filename).into_bytes());
    }
    if mime == "application/pdf" || lower.ends_with(".pdf") {
        return ("application/pdf".into(), demo_pdf(filename));
    }
    if mime.starts_with("text/") {
        return ("text/plain".into(), format!("{filename}\n\nThis is a demo file.\n").into_bytes());
    }
    if lower.ends_with(".zip") {
        let mut eocd = vec![0x50, 0x4b, 0x05, 0x06];
        eocd.extend_from_slice(&[0; 18]);
        return ("application/zip".into(), eocd);
    }
    ("application/octet-stream".into(), format!("Demo file: {filename}\n").into_bytes())
}

fn demo_image(filename: &str) -> String {
    let label = html_escape::encode_text(filename);
    if filename.to_lowercase().contains("logo") {
        return format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 192 64\"><rect width=\"192\" height=\"64\" fill=\"#ffcc00\"/>\
             <text x=\"96\" y=\"44\" text-anchor=\"middle\" font-family=\"sans-serif\" font-weight=\"800\" font-size=\"36\" fill=\"#d40511\">{}</text></svg>",
            label.split('-').next().unwrap_or("LOGO").to_uppercase()
        );
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 640 400\"><rect width=\"640\" height=\"400\" fill=\"#c9d6e0\"/>\
         <circle cx=\"490\" cy=\"118\" r=\"46\" fill=\"#f2f6f9\"/><path d=\"M0 400 L170 196 L292 326 L404 226 L640 400Z\" fill=\"#7f97aa\"/>\
         <path d=\"M0 400 L120 300 L230 400Z\" fill=\"#566d80\"/>\
         <text x=\"24\" y=\"374\" font-family=\"sans-serif\" font-size=\"20\" fill=\"#172135\">{label}</text></svg>"
    )
}

fn demo_pdf(filename: &str) -> Vec<u8> {
    let title: String =
        filename.chars().map(|c| if c.is_ascii() && c != '(' && c != ')' && c != '\\' { c } else { '?' }).collect();
    let content =
        format!("BT /F1 26 Tf 72 740 Td ({title}) Tj 0 -40 Td /F1 14 Tf (Demo file. Nothing real is in here.) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use jiff::tz::TimeZone;
    use zsm_core::clock::FixedClock;
    use zsm_core::dto::PeopleFilter;

    use super::*;
    use crate::{Core, CoreConfig};

    fn core() -> Arc<Core> {
        let core = Core::new(CoreConfig {
            clock: Arc::new(FixedClock::new(1_795_082_400_000)),
            timezone: TimeZone::UTC,
            ..CoreConfig::in_memory()
        })
        .unwrap();
        core.start_demo(0).unwrap();
        core
    }

    fn dhl_first_message(core: &Core) -> (i64, i64) {
        let dhl = core.people(PeopleFilter::All, "DHL").unwrap()[0].id;
        let convo =
            core.conversations(dhl).unwrap().into_iter().find(|c| c.subject == "Your parcel is on its way").unwrap();
        let detail = core.conversation(convo.id, Some(dhl)).unwrap().unwrap();
        let first = &detail.messages[0];
        (first.id, first.attachments.iter().find(|a| a.inline).unwrap().id)
    }

    fn header<'a>(served: &'a Served, name: &str) -> &'a str {
        served.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).unwrap_or("")
    }

    #[test]
    fn a_message_is_served_as_a_locked_down_document() {
        let core = core();
        let (message, _) = dhl_first_message(&core);
        let served = core.serve(&format!("msg/{message}"), None);
        assert_eq!(served.status, 200);
        assert_eq!(served.content_type, "text/html; charset=utf-8");
        let body = String::from_utf8(served.body.clone()).unwrap();
        let csp = header(&served, "Content-Security-Policy");
        assert!(csp.starts_with("default-src 'none'; script-src 'nonce-"));
        assert!(csp.contains("img-src 'self' data:;") && !csp.contains("https:"), "remote images are off by default");
        assert!(!csp.contains("unsafe-eval") && csp.contains("frame-src 'none'") && csp.contains("form-action 'none'"));
        let nonce = csp.split("'nonce-").nth(1).unwrap().split('\'').next().unwrap();
        assert_eq!(nonce.len(), 32);
        assert_eq!(body.matches(&format!("nonce=\"{nonce}\"")).count(), 1, "exactly one script, and it is ours");
        assert_eq!(body.matches("<script").count(), 1, "the message's own script was removed");
        assert!(body.contains("Your order <b>394-2211</b>"));
        assert!(body.contains("data-zsm-src="), "remote images are held back until the reader asks");
        assert!(!body.contains(" src=\"https://cdn.parcel"), "no live remote src remains");
        assert_eq!(header(&served, "Cache-Control"), "no-store");
    }

    #[test]
    fn remote_images_load_only_when_asked_and_only_over_https() {
        let core = core();
        let (message, _) = dhl_first_message(&core);
        let served = core.serve(&format!("msg/{message}"), Some("remote=1"));
        let body = String::from_utf8(served.body.clone()).unwrap();
        assert!(body.contains("src=\"https://cdn.parcel.dhl.example/banner/summer.jpg\""));
        assert!(header(&served, "Content-Security-Policy").contains("img-src 'self' data: https:;"));
    }

    #[test]
    fn inline_images_are_rewritten_to_relative_protocol_urls_and_served() {
        let core = core();
        let (message, logo) = dhl_first_message(&core);
        let body = String::from_utf8(core.serve(&format!("msg/{message}"), None).body).unwrap();
        assert!(body.contains(&format!("src=\"../cid/{logo}\"")), "{body}");
        let image = core.serve(&format!("cid/{logo}"), None);
        assert_eq!(image.status, 200);
        assert_eq!(image.content_type, "image/svg+xml");
        assert!(header(&image, "Content-Security-Policy").starts_with("sandbox;"), "an SVG cannot run anything");
        assert!(String::from_utf8(image.body).unwrap().contains("DHL"));
    }

    #[test]
    fn attachments_download_unless_preview_is_asked_for_and_safe() {
        let core = core();
        let maya = core.people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let files = core.files(maya).unwrap();
        let pdf = files.iter().find(|f| f.attachment.name == "Invoice-2291.pdf").unwrap().attachment.id;
        let download = core.serve(&format!("att/{pdf}"), None);
        assert_eq!(download.content_type, "application/octet-stream");
        assert!(header(&download, "Content-Disposition").starts_with("attachment; filename*=UTF-8''Invoice-2291.pdf"));
        let preview = core.serve(&format!("att/{pdf}"), Some("inline=1"));
        assert_eq!(preview.content_type, "application/pdf");
        assert!(preview.body.starts_with(b"%PDF-1.4") && preview.body.ends_with(b"%%EOF\n"));
        assert!(header(&preview, "Content-Disposition").starts_with("inline;"));
        let zip = files.iter().find(|f| f.attachment.name == "screenshots-v3.zip").unwrap().attachment.id;
        assert_eq!(core.serve(&format!("att/{zip}"), Some("inline=1")).content_type, "application/octet-stream");
    }

    #[test]
    fn saving_a_file_never_overwrites_and_never_escapes_the_folder() {
        let dir = std::env::temp_dir().join(format!("zsm-save-{:x}", rand::random::<u64>()));
        let core = Core::new(CoreConfig {
            clock: Arc::new(FixedClock::new(1_795_082_400_000)),
            timezone: TimeZone::UTC,
            downloads_dir: Some(dir.clone()),
            ..CoreConfig::in_memory()
        })
        .unwrap();
        core.start_demo(0).unwrap();
        let maya = core.people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let pdf = core.files(maya).unwrap().into_iter().find(|f| f.attachment.name == "Invoice-2291.pdf").unwrap();
        let first = core.save_attachment(pdf.attachment.id).unwrap();
        let second = core.save_attachment(pdf.attachment.id).unwrap();
        assert!(first.ends_with("Invoice-2291.pdf") && second.ends_with("Invoice-2291 (1).pdf"), "{first} {second}");
        assert!(std::fs::read(&first).unwrap().starts_with(b"%PDF"));
        assert!(core.save_attachment(999_999).is_err());
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("  .hidden "), "hidden");
        assert_eq!(safe_file_name(""), "attachment");
        assert_eq!(safe_file_name("CON"), "_CON");
        assert_eq!(safe_file_name("nul.txt"), "_nul.txt");
        assert_eq!(safe_file_name("Com1.pdf"), "_Com1.pdf");
        assert_eq!(safe_file_name("lpt9"), "_lpt9");
        assert_eq!(safe_file_name("report. . "), "report");
        assert_eq!(safe_file_name("console.txt"), "console.txt");
        assert_eq!(safe_file_name("COM10.txt"), "COM10.txt");
        assert_eq!(safe_file_name("a:b*c?.txt"), "a_b_c_.txt");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_or_malformed_paths_are_a_plain_404() {
        let core = core();
        for path in ["msg/999999", "msg/abc", "nope/1", "", "cid/", "att/-1"] {
            let served = core.serve(path, None);
            assert_eq!(served.status, 404, "{path}");
            assert_eq!(served.content_type, "text/plain; charset=utf-8");
        }
    }

    #[test]
    fn the_demo_pdf_has_a_consistent_cross_reference_table() {
        let pdf = demo_pdf("Quarterly (draft).pdf");
        let text = String::from_utf8_lossy(&pdf).to_string();
        let start: usize = text.rsplit("startxref\n").next().unwrap().split('\n').next().unwrap().parse().unwrap();
        assert!(text[start..].starts_with("xref"), "startxref points at the table");
        let first_object: usize =
            text.split("xref\n0 6\n").nth(1).unwrap().lines().nth(1).unwrap()[..10].parse().unwrap();
        assert!(text[first_object..].starts_with("1 0 obj"));
        assert!(!text.contains("(draft)"), "parentheses in names are escaped out of the content stream");
    }
}
