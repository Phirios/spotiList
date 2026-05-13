import os
import random
from typing import List, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from yt_dlp import YoutubeDL

app = FastAPI(title="spoti yt-scraper")


def _proxy_pool() -> list[str]:
    raw = os.getenv("YT_PROXIES", "").strip()
    if not raw:
        return []
    return [p.strip() for p in raw.split(",") if p.strip()]


def _ydl_opts(extra: dict | None = None) -> dict:
    opts: dict = {
        "quiet": True,
        "no_warnings": True,
        "skip_download": True,
        "extract_flat": False,
        "socket_timeout": 30,
    }
    pool = _proxy_pool()
    if pool:
        opts["proxy"] = random.choice(pool)
    if extra:
        opts.update(extra)
    return opts


class MatchRequest(BaseModel):
    title: str = Field(..., min_length=1)
    artist: str = Field(..., min_length=1)
    duration_sec: Optional[int] = Field(default=None, ge=1)


class MatchResponse(BaseModel):
    video_id: str
    title: str
    channel: str
    duration_sec: Optional[int]
    score: float


def _score_candidate(c: dict, target_artist: str, target_dur: Optional[int]) -> float:
    score = 0.0
    title = (c.get("title") or "").lower()
    channel = (c.get("channel") or c.get("uploader") or "").lower()
    artist = target_artist.lower()
    if artist in channel:
        score += 2.0
    if "vevo" in channel:
        score += 1.5
    if "official" in title or "official audio" in title:
        score += 1.0
    if "lyric" in title or "lyrics" in title:
        score -= 0.3
    if "cover" in title or "remix" in title:
        score -= 0.7
    if target_dur and c.get("duration"):
        delta = abs(int(c["duration"]) - target_dur)
        if delta <= 3:
            score += 1.5
        elif delta <= 10:
            score += 0.7
        elif delta > 30:
            score -= 0.5
    return score


@app.post("/match", response_model=MatchResponse)
def match(req: MatchRequest) -> MatchResponse:
    query = f"ytsearch5:{req.title} {req.artist}"
    try:
        with YoutubeDL(_ydl_opts({"extract_flat": "in_playlist"})) as ydl:
            info = ydl.extract_info(query, download=False)
    except Exception as e:
        raise HTTPException(status_code=502, detail=f"yt-dlp search failed: {e}") from e

    entries = (info or {}).get("entries") or []
    if not entries:
        raise HTTPException(status_code=404, detail="no candidates")

    ranked = sorted(
        entries,
        key=lambda c: _score_candidate(c, req.artist, req.duration_sec),
        reverse=True,
    )
    best = ranked[0]
    return MatchResponse(
        video_id=best["id"],
        title=best.get("title") or "",
        channel=best.get("channel") or best.get("uploader") or "",
        duration_sec=best.get("duration"),
        score=_score_candidate(best, req.artist, req.duration_sec),
    )


class CommentsRequest(BaseModel):
    video_id: str = Field(..., min_length=5)
    max_comments: int = Field(default=200, ge=1, le=2000)


class Comment(BaseModel):
    text: str
    likes: int = 0


class CommentsResponse(BaseModel):
    video_id: str
    comments: List[Comment]


@app.post("/comments", response_model=CommentsResponse)
def comments(req: CommentsRequest) -> CommentsResponse:
    url = f"https://www.youtube.com/watch?v={req.video_id}"
    extra = {
        "getcomments": True,
        "extractor_args": {
            "youtube": {"max_comments": [str(req.max_comments), "all", "all", "all"]}
        },
    }
    try:
        with YoutubeDL(_ydl_opts(extra)) as ydl:
            info = ydl.extract_info(url, download=False)
    except Exception as e:
        raise HTTPException(status_code=502, detail=f"yt-dlp failed: {e}") from e

    raw = (info or {}).get("comments") or []
    out: List[Comment] = []
    for c in raw[: req.max_comments]:
        text = (c.get("text") or "").strip()
        if not text:
            continue
        out.append(Comment(text=text, likes=int(c.get("like_count") or 0)))
    return CommentsResponse(video_id=req.video_id, comments=out)


@app.get("/healthz")
def healthz() -> dict:
    return {"ok": True, "proxies": len(_proxy_pool())}
