// Files the player attaches to a message (docs/design.md#attachments):
// pictures, PDFs (sheet music, notes), videos, sound files and text. Each
// is read when attached, into what the model can take: words, and
// pictures scaled down to JPEG. They wait beside the input until the
// message is sent, then go with it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageReader, RgbImage, codecs::jpeg::JpegEncoder};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use serde_json::{Value, json};
use tauri::{AppHandle, State};

use crate::audio_file;
use crate::commands;

// The most files waiting at once.
pub const MOST_ATTACHMENTS: usize = 8;
// The longest side a picture is sent at, in pixels: past it, models scale
// pictures down themselves.
const LONGEST_SIDE: u32 = 1568;
// How good the JPEGs are, out of 100.
const JPEG_QUALITY: u8 = 85;
// The most PDF pages read, and the longest side each page is drawn at.
const MOST_PAGES: usize = 20;
const PAGE_LONGEST_SIDE: u32 = 1024;
// The most words, in characters, taken from one file.
const MOST_TEXT_CHARACTERS: usize = 40_000;
// How many pictures a video gives, and the longest side of each.
const VIDEO_FRAMES: usize = 12;
const FRAME_LONGEST_SIDE: u32 = 1024;
// The largest file read, in bytes: 2 GB.
const LARGEST_FILE: u64 = 2_000_000_000;

const IMAGE_ENDINGS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
const AUDIO_ENDINGS: [&str; 10] = ["wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "aiff", "aif", "wma"];
const VIDEO_ENDINGS: [&str; 7] = ["mp4", "mov", "m4v", "webm", "mkv", "avi", "wmv"];
const TEXT_ENDINGS: [&str; 7] = ["txt", "md", "csv", "json", "lrc", "abc", "xml"];

const NO_PDFIUM: &str = "Jello needs the PDFium library to read PDFs. Put it in Jello's settings folder under pdfium, or set JELLO_PDFIUM to where it is.";

// A picture ready to send: a JPEG, as base64.
#[derive(Debug, Clone)]
pub struct Picture {
    pub data: String,
}

// One file read and waiting.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: u32,
    pub name: String,
    pub path: PathBuf,
    pub kind: &'static str,
    // What the model reads about it.
    pub text: String,
    pub pictures: Vec<Picture>,
    // What the card under the input says was taken, like "3 pages".
    pub taken: String,
}

impl Attachment {
    // The card the panel shows.
    pub fn card(&self) -> Value {
        json!({"id": self.id, "name": self.name, "kind": self.kind, "taken": self.taken})
    }

    // What goes to the host with the message.
    pub fn message(&self) -> Value {
        let mut images = Vec::new();
        for picture in &self.pictures {
            images.push(json!({"data": picture.data, "mimeType": "image/jpeg"}));
        }
        json!({
            "name": self.name,
            "path": self.path.to_string_lossy(),
            "kind": self.kind,
            "text": self.text,
            "images": images,
        })
    }
}

// The files waiting to go with the next message.
#[derive(Default)]
pub struct Attachments {
    next_id: u32,
    waiting: Vec<Attachment>,
}

impl Attachments {
    pub fn add(&mut self, mut attachment: Attachment) -> Result<(), String> {
        if self.waiting.len() >= MOST_ATTACHMENTS {
            return Err(format!("a message takes at most {MOST_ATTACHMENTS} files"));
        }
        self.next_id += 1;
        attachment.id = self.next_id;
        self.waiting.push(attachment);
        Ok(())
    }

    pub fn remove(&mut self, id: u32) {
        self.waiting.retain(|attachment| attachment.id != id);
    }

    pub fn cards(&self) -> Vec<Value> {
        let mut cards = Vec::new();
        for attachment in &self.waiting {
            cards.push(attachment.card());
        }
        cards
    }

    // Every file waiting, for the message, leaving none.
    pub fn take(&mut self) -> Vec<Attachment> {
        std::mem::take(&mut self.waiting)
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

// The kind of file `path` is, from its ending, or None.
pub fn kind_of(path: &Path) -> Option<&'static str> {
    let ending = path.extension()?.to_str()?.to_lowercase();
    if IMAGE_ENDINGS.contains(&ending.as_str()) {
        return Some("image");
    }
    if ending == "pdf" {
        return Some("pdf");
    }
    if AUDIO_ENDINGS.contains(&ending.as_str()) {
        return Some("audio");
    }
    if VIDEO_ENDINGS.contains(&ending.as_str()) {
        return Some("video");
    }
    if TEXT_ENDINGS.contains(&ending.as_str()) {
        return Some("text");
    }
    None
}

// Reads the file at `path` into an attachment. `settings` is Jello's
// settings folder, where PDFium may be.
pub fn read(path: &Path, settings: Option<&Path>) -> Result<Attachment, String> {
    let name = match path.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => return Err(String::from("that isn't a file")),
    };
    let size = fs::metadata(path).map_err(|error| format!("{name} can't be read: {error}"))?.len();
    if size > LARGEST_FILE {
        return Err(format!("{name} is larger than 2 GB"));
    }
    let Some(kind) = kind_of(path) else {
        return Err(format!("Jello can't read {name}: it takes pictures, PDFs, videos, sound files and text"));
    };
    let mut attachment = Attachment {
        id: 0,
        name: name.clone(),
        path: path.to_path_buf(),
        kind,
        text: String::new(),
        pictures: Vec::new(),
        taken: String::new(),
    };
    match kind {
        "image" => read_image(path, &mut attachment)?,
        "pdf" => read_pdf(path, settings, &mut attachment)?,
        "audio" => read_audio(path, &mut attachment)?,
        "video" => read_video(path, &mut attachment)?,
        _ => read_text(path, &mut attachment)?,
    }
    Ok(attachment)
}

// `image` scaled so its longest side is at most `longest`, on white where
// it is see-through, as a JPEG.
pub fn picture_of(image: &DynamicImage, longest: u32) -> Result<Picture, String> {
    let mut scaled = image.clone();
    if image.width().max(image.height()) > longest {
        scaled = image.resize(longest, longest, image::imageops::FilterType::Triangle);
    }
    let rgba = scaled.to_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let mut color = [0_u8; 3];
        for channel in 0..3 {
            let shown = f32::from(pixel[channel]) * alpha + 255.0 * (1.0 - alpha);
            color[channel] = shown.round() as u8;
        }
        rgb.put_pixel(x, y, image::Rgb(color));
    }
    let mut bytes = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
    rgb.write_with_encoder(encoder).map_err(|error| format!("the picture couldn't be made: {error}"))?;
    Ok(Picture {
        data: STANDARD.encode(&bytes),
    })
}

fn read_image(path: &Path, attachment: &mut Attachment) -> Result<(), String> {
    let reader = ImageReader::open(path).map_err(|error| format!("{} can't be read: {error}", attachment.name))?;
    let reader = reader.with_guessed_format().map_err(|error| format!("{} can't be read: {error}", attachment.name))?;
    let image = reader.decode().map_err(|error| format!("{} isn't a picture Jello can read: {error}", attachment.name))?;
    let picture = picture_of(&image, LONGEST_SIDE)?;
    attachment.text = format!("A picture, {} by {} pixels.", image.width(), image.height());
    attachment.taken = format!("{} by {}", image.width(), image.height());
    attachment.pictures.push(picture);
    Ok(())
}

// Where PDFium is: JELLO_PDFIUM (the library or its folder), then the
// pdfium folder in Jello's settings, then the system's.
fn pdfium_places(settings: Option<&Path>) -> Vec<PathBuf> {
    let mut places = Vec::new();
    if let Ok(place) = std::env::var("JELLO_PDFIUM") {
        let place = PathBuf::from(place);
        if place.is_dir() {
            places.push(Pdfium::pdfium_platform_library_name_at_path(&place));
        } else {
            places.push(place);
        }
    }
    if let Some(settings) = settings {
        places.push(Pdfium::pdfium_platform_library_name_at_path(&settings.join("pdfium")));
    }
    places
}

// PDFium, bound once and kept. Not bound yet while it can't be found, so
// putting it in place works without starting Jello again.
static PDFIUM: Mutex<Option<Pdfium>> = Mutex::new(None);

fn with_pdfium<T>(settings: Option<&Path>, work: impl FnOnce(&Pdfium) -> Result<T, String>) -> Result<T, String> {
    let mut kept = PDFIUM.lock().unwrap();
    if kept.is_none() {
        for place in pdfium_places(settings) {
            if !place.is_file() {
                continue;
            }
            if let Ok(bindings) = Pdfium::bind_to_library(&place) {
                *kept = Some(Pdfium::new(bindings));
                break;
            }
        }
    }
    if kept.is_none() {
        if let Ok(bindings) = Pdfium::bind_to_system_library() {
            *kept = Some(Pdfium::new(bindings));
        }
    }
    match kept.as_ref() {
        Some(pdfium) => work(pdfium),
        None => Err(String::from(NO_PDFIUM)),
    }
}

fn read_pdf(path: &Path, settings: Option<&Path>, attachment: &mut Attachment) -> Result<(), String> {
    let name = attachment.name.clone();
    let (pages, text, pictures) = with_pdfium(settings, |pdfium| {
        let document = pdfium
            .load_pdf_from_file(path, None)
            .map_err(|error| format!("{name} can't be opened: {error}"))?;
        let pages = document.pages();
        let page_count = pages.len().max(0) as usize;
        let config = PdfRenderConfig::new()
            .set_maximum_width(PAGE_LONGEST_SIDE as i32)
            .set_maximum_height(PAGE_LONGEST_SIDE as i32);
        let mut text = String::new();
        let mut pictures = Vec::new();
        for (place, page) in pages.iter().enumerate() {
            if place >= MOST_PAGES {
                break;
            }
            let mut words = String::new();
            if let Ok(page_text) = page.text() {
                words = page_text.all();
            }
            text.push_str(&format!("Page {}:\n{}\n\n", place + 1, words.trim()));
            let drawn = page
                .render_with_config(&config)
                .map_err(|error| format!("page {} of {name} can't be drawn: {error}", place + 1))?;
            let image = drawn
                .as_image()
                .map_err(|error| format!("page {} of {name} can't be drawn: {error}", place + 1))?;
            pictures.push(picture_of(&image, PAGE_LONGEST_SIDE)?);
        }
        Ok((page_count, text, pictures))
    })?;

    let shown = pictures.len();
    let mut about = format!("A PDF of {}, each as a picture and its words.", pages_word(pages));
    if shown < pages {
        about = format!("A PDF of {}; the first {shown} are here, each as a picture and its words.", pages_word(pages));
    }
    attachment.text = format!("{about}\n\n{}", shortened(&text));
    attachment.taken = format!("{shown} of {}", pages_word(pages));
    if shown == pages {
        attachment.taken = pages_word(pages);
    }
    attachment.pictures = pictures;
    Ok(())
}

fn pages_word(pages: usize) -> String {
    if pages == 1 {
        return String::from("1 page");
    }
    format!("{pages} pages")
}

fn read_audio(path: &Path, attachment: &mut Attachment) -> Result<(), String> {
    let samples = audio_file::samples_of(path)?;
    let reading = audio_file::reading_of(&samples, audio_file::RATE);
    attachment.taken = format!("{} seconds heard", reading["seconds"]);
    attachment.text = format!(
        "A sound file, as Jello heard it (the first {} seconds at most; the tempo, key and tune are guesses):\n{}",
        audio_file::MOST_SECONDS,
        serde_json::to_string_pretty(&reading).unwrap_or_default()
    );
    Ok(())
}

// How long `path` plays, in seconds, from FFprobe.
fn seconds_of(path: &Path) -> Result<f64, String> {
    let Some(ffprobe) = audio_file::program("ffprobe") else {
        return Err(String::from(audio_file::NO_FFMPEG));
    };
    let output = Command::new(ffprobe)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path)
        .output()
        .map_err(|error| format!("FFprobe didn't start: {error}"))?;
    let said = String::from_utf8_lossy(&output.stdout);
    let Ok(seconds) = said.trim().parse::<f64>() else {
        return Err(String::from("FFprobe couldn't tell how long the video is"));
    };
    if seconds <= 0.0 {
        return Err(String::from("FFprobe couldn't tell how long the video is"));
    }
    Ok(seconds)
}

fn read_video(path: &Path, attachment: &mut Attachment) -> Result<(), String> {
    let Some(ffmpeg) = audio_file::program("ffmpeg") else {
        return Err(String::from(audio_file::NO_FFMPEG));
    };
    let seconds = seconds_of(path)?;
    let folder = std::env::temp_dir().join(format!("jello-frames-{}-{}", std::process::id(), unique_number()));
    fs::create_dir_all(&folder).map_err(|error| format!("the video's pictures have nowhere to go: {error}"))?;

    // Pictures spread evenly through it, at most one a second.
    let mut rate = VIDEO_FRAMES as f64 / seconds;
    if rate > 1.0 {
        rate = 1.0;
    }
    let output = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vf", &format!("fps={rate:.6}"), "-frames:v", &VIDEO_FRAMES.to_string(), "-q:v", "3"])
        .arg(folder.join("frame-%02d.jpg"))
        .output()
        .map_err(|error| format!("FFmpeg didn't start: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_dir_all(&folder);
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg couldn't read the video: {}", said.trim()));
    }

    let mut frames = Vec::new();
    for number in 1..=VIDEO_FRAMES {
        let frame_path = folder.join(format!("frame-{number:02}.jpg"));
        let Ok(image) = image::open(&frame_path) else {
            continue;
        };
        frames.push(picture_of(&image, FRAME_LONGEST_SIDE)?);
    }
    let _ = fs::remove_dir_all(&folder);

    let mut sound = String::from("It has no sound.");
    if let Ok(samples) = audio_file::samples_of(path) {
        if !samples.is_empty() {
            let reading = audio_file::reading_of(&samples, audio_file::RATE);
            sound = format!(
                "Its sound, as Jello heard it (the tempo, key and tune are guesses):\n{}",
                serde_json::to_string_pretty(&reading).unwrap_or_default()
            );
        }
    }
    let gap = seconds / frames.len().max(1) as f64;
    attachment.text = format!(
        "A video of {seconds:.1} seconds, as {} pictures about {gap:.1} seconds apart. {sound}",
        frames.len()
    );
    attachment.taken = format!("{} pictures and its sound", frames.len());
    attachment.pictures = frames;
    Ok(())
}

fn unique_number() -> u128 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    match now {
        Ok(since) => since.as_nanos(),
        Err(_) => 0,
    }
}

fn read_text(path: &Path, attachment: &mut Attachment) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("{} can't be read: {error}", attachment.name))?;
    let text = String::from_utf8_lossy(&bytes);
    let characters = text.chars().count();
    attachment.text = format!("A text file:\n{}", shortened(&text));
    attachment.taken = format!("{characters} characters");
    if characters > MOST_TEXT_CHARACTERS {
        attachment.taken = format!("the first {MOST_TEXT_CHARACTERS} of {characters} characters");
    }
    Ok(())
}

// `text` cut to MOST_TEXT_CHARACTERS, saying so when it was cut.
fn shortened(text: &str) -> String {
    if text.chars().count() <= MOST_TEXT_CHARACTERS {
        return text.to_string();
    }
    let kept: String = text.chars().take(MOST_TEXT_CHARACTERS).collect();
    format!("{kept}\n(The rest was cut.)")
}

// The player attaches the files at `paths`, from the paperclip or a drop.
// Every file is read before any is added, so one that fails adds none.
// Gives every card waiting.
#[tauri::command(async)]
pub fn agent_attach(app: AppHandle, attachments: State<'_, Mutex<Attachments>>, paths: Vec<String>) -> Result<Vec<Value>, String> {
    let settings = commands::settings_folder(&app).ok();
    let mut read_ones = Vec::new();
    for path in paths {
        read_ones.push(read(Path::new(&path), settings.as_deref())?);
    }
    let mut waiting = attachments.lock().unwrap();
    for attachment in read_ones {
        waiting.add(attachment)?;
    }
    Ok(waiting.cards())
}

// The player takes file `id` off the next message.
#[tauri::command]
pub fn agent_unattach(attachments: State<'_, Mutex<Attachments>>, id: u32) -> Vec<Value> {
    let mut waiting = attachments.lock().unwrap();
    waiting.remove(id);
    waiting.cards()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::test_folder;
    use image::{Rgba, RgbaImage};

    #[test]
    fn files_are_known_by_their_ending() {
        assert_eq!(kind_of(Path::new("a/Score.PDF")), Some("pdf"));
        assert_eq!(kind_of(Path::new("take.m4a")), Some("audio"));
        assert_eq!(kind_of(Path::new("clip.mov")), Some("video"));
        assert_eq!(kind_of(Path::new("shot.webp")), Some("image"));
        assert_eq!(kind_of(Path::new("lyrics.txt")), Some("text"));
        assert_eq!(kind_of(Path::new("app.exe")), None);
        assert_eq!(kind_of(Path::new("no-ending")), None);
    }

    #[test]
    fn a_picture_is_scaled_and_flattened() {
        let folder = test_folder("attach-image");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("wide.png");
        let mut image = RgbaImage::new(3000, 1000);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
        image.save(&path).unwrap();

        let attachment = read(&path, None).unwrap();
        assert_eq!(attachment.kind, "image");
        assert_eq!(attachment.taken, "3000 by 1000");
        let picture = &attachment.pictures[0];
        let bytes = STANDARD.decode(&picture.data).unwrap();
        let back = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(back.dimensions(), (1568, 523));
        // See-through became white.
        assert!(back.get_pixel(10, 10)[0] > 240);
    }

    #[test]
    fn text_is_read_and_long_text_cut() {
        let folder = test_folder("attach-text");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("lyrics.txt");
        fs::write(&path, "Verse one\nla la").unwrap();
        let attachment = read(&path, None).unwrap();
        assert!(attachment.text.ends_with("Verse one\nla la"));
        assert_eq!(attachment.taken, "15 characters");

        let long = folder.join("long.md");
        fs::write(&long, "a".repeat(MOST_TEXT_CHARACTERS + 5)).unwrap();
        let attachment = read(&long, None).unwrap();
        assert!(attachment.text.ends_with("(The rest was cut.)"));
        assert!(attachment.taken.starts_with("the first"));
    }

    #[test]
    fn unknown_and_missing_files_are_refused() {
        let folder = test_folder("attach-refused");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("tool.exe");
        fs::write(&path, "x").unwrap();
        assert!(read(&path, None).unwrap_err().contains("can't read tool.exe"));
        assert!(read(&folder.join("gone.png"), None).unwrap_err().contains("can't be read"));
    }

    #[test]
    fn at_most_eight_wait_and_sending_takes_them() {
        let mut waiting = Attachments::default();
        let file = Attachment {
            id: 0,
            name: String::from("a.txt"),
            path: PathBuf::from("a.txt"),
            kind: "text",
            text: String::from("hi"),
            pictures: Vec::new(),
            taken: String::new(),
        };
        for _ in 0..MOST_ATTACHMENTS {
            waiting.add(file.clone()).unwrap();
        }
        assert!(waiting.add(file.clone()).is_err());
        waiting.remove(3);
        assert_eq!(waiting.cards().len(), MOST_ATTACHMENTS - 1);
        assert_eq!(waiting.cards()[2]["id"], 4);
        let taken = waiting.take();
        assert_eq!(taken.len(), MOST_ATTACHMENTS - 1);
        assert!(waiting.is_empty());
        assert_eq!(taken[0].message()["kind"], "text");
    }

    #[test]
    fn a_video_gives_pictures_and_its_sound() {
        let Some(ffmpeg) = audio_file::program("ffmpeg") else {
            return;
        };
        if audio_file::program("ffprobe").is_none() {
            return;
        }
        let folder = test_folder("attach-video");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("clip.mp4");
        let status = Command::new(ffmpeg)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=duration=4:size=320x240:rate=10"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=4", "-shortest"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let attachment = read(&path, None).unwrap();
        assert_eq!(attachment.kind, "video");
        assert_eq!(attachment.pictures.len(), 4);
        assert!(attachment.text.contains("A4"), "{}", attachment.text);
        assert_eq!(attachment.taken, "4 pictures and its sound");
    }

    // A two page PDF written by hand, with words on each page. PDFium
    // mends the rough cross reference table.
    fn small_pdf() -> String {
        let mut pdf = String::from("%PDF-1.4\n");
        pdf.push_str("1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n");
        pdf.push_str("2 0 obj << /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >> endobj\n");
        let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 7 0 R >> >> /Contents";
        pdf.push_str(&format!("3 0 obj {page} 4 0 R >> endobj\n"));
        let first = "BT /F1 18 Tf 20 100 Td (Verse in C major) Tj ET";
        pdf.push_str(&format!("4 0 obj << /Length {} >> stream\n{first}\nendstream endobj\n", first.len()));
        pdf.push_str(&format!("5 0 obj {page} 6 0 R >> endobj\n"));
        let second = "BT /F1 18 Tf 20 100 Td (Chorus) Tj ET";
        pdf.push_str(&format!("6 0 obj << /Length {} >> stream\n{second}\nendstream endobj\n", second.len()));
        pdf.push_str("7 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n");
        pdf.push_str("trailer << /Root 1 0 R /Size 8 >>\n%%EOF\n");
        pdf
    }

    #[test]
    fn a_pdf_gives_each_page_as_a_picture_and_its_words() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let settings = PathBuf::from(home).join("Library/Application Support/com.jello.app");
        let folder = test_folder("attach-pdf");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Song.pdf");
        fs::write(&path, small_pdf()).unwrap();
        let attachment = match read(&path, Some(&settings)) {
            Ok(attachment) => attachment,
            Err(error) if error == NO_PDFIUM => return,
            Err(error) => panic!("{error}"),
        };
        assert_eq!(attachment.kind, "pdf");
        assert_eq!(attachment.taken, "2 pages");
        assert_eq!(attachment.pictures.len(), 2);
        assert!(attachment.text.contains("Page 1:\nVerse in C major"), "{}", attachment.text);
        assert!(attachment.text.contains("Page 2:\nChorus"), "{}", attachment.text);
    }
}
