import os
from contextlib import asynccontextmanager
from typing import List

from fastapi import FastAPI
from pydantic import BaseModel, Field
from sentence_transformers import SentenceTransformer

MODEL_NAME = os.getenv("MODEL_NAME", "sentence-transformers/all-MiniLM-L6-v2")

state: dict[str, SentenceTransformer | None] = {"model": None}


@asynccontextmanager
async def lifespan(_: FastAPI):
    state["model"] = SentenceTransformer(MODEL_NAME)
    yield
    state["model"] = None


app = FastAPI(title="spoti embedder", lifespan=lifespan)


class EmbedRequest(BaseModel):
    texts: List[str] = Field(..., min_length=1, max_length=256)
    normalize: bool = True


class EmbedResponse(BaseModel):
    model: str
    dim: int
    embeddings: List[List[float]]


@app.get("/health")
def health():
    return {"ok": True, "model": MODEL_NAME, "ready": state["model"] is not None}


@app.post("/embed", response_model=EmbedResponse)
def embed(req: EmbedRequest):
    model = state["model"]
    assert model is not None, "model not loaded"
    vectors = model.encode(
        req.texts,
        normalize_embeddings=req.normalize,
        convert_to_numpy=True,
        show_progress_bar=False,
    )
    return EmbedResponse(
        model=MODEL_NAME,
        dim=int(vectors.shape[1]),
        embeddings=vectors.tolist(),
    )
