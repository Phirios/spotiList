use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TrackInfo {
    pub id: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration_ms: u64,
    pub explicit: bool,
    pub popularity: Option<u8>,
    pub isrc: Option<String>,
    pub spotify_url: Option<String>,
    pub genres: Vec<String>,
    pub bpm: Option<Bpm>,
    pub lyrics: Option<Lyrics>,
}

#[derive(Debug, Serialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub release_date: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Bpm {
    pub tempo: f64,
    pub source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Lyrics {
    pub plain: Option<String>,
    pub synced: Option<String>,
    pub instrumental: bool,
    pub source: &'static str,
}
