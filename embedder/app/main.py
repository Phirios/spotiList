import math
import os
from contextlib import asynccontextmanager
from typing import List, Optional

import numpy as np
from fastapi import FastAPI
from pydantic import BaseModel, Field
from sentence_transformers import SentenceTransformer
from sklearn.cluster import KMeans

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


class ClusterRequest(BaseModel):
    embeddings: List[List[float]] = Field(..., min_length=2)
    k: Optional[int] = Field(default=None, ge=2, le=50)
    target_per_cluster: int = Field(default=25, ge=5, le=100)


class ClusterResponse(BaseModel):
    k: int
    labels: List[int]
    centroids: List[List[float]]


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


@app.post("/cluster", response_model=ClusterResponse)
def cluster(req: ClusterRequest):
    x = np.asarray(req.embeddings, dtype=np.float32)
    n = x.shape[0]
    if req.k is not None:
        k = max(2, min(req.k, n - 1))
    else:
        # ceil(n / target_per_cluster), clamped to [3, 15]
        k = max(3, min(15, math.ceil(n / req.target_per_cluster)))
        k = min(k, n - 1)

    km = KMeans(n_clusters=k, n_init=10, random_state=42)
    labels = km.fit_predict(x)
    return ClusterResponse(
        k=k,
        labels=labels.tolist(),
        centroids=km.cluster_centers_.tolist(),
    )
