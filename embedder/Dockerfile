FROM python:3.12-slim AS runtime

ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1 \
    HF_HOME=/cache/hf \
    SENTENCE_TRANSFORMERS_HOME=/cache/hf

WORKDIR /app

# Install CPU-only torch wheel first (smaller than the default which pulls CUDA)
RUN pip install --upgrade pip \
 && pip install --extra-index-url https://download.pytorch.org/whl/cpu torch==2.6.0+cpu

COPY requirements.txt .
RUN pip install -r requirements.txt

COPY app ./app

# Pre-download the models so first request isn't slow
ARG MODEL_NAME=sentence-transformers/all-MiniLM-L6-v2
ARG EMOTION_MODEL=j-hartmann/emotion-english-distilroberta-base
ENV MODEL_NAME=${MODEL_NAME} \
    EMOTION_MODEL=${EMOTION_MODEL}
RUN python -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('${MODEL_NAME}')" \
 && python -c "from transformers import pipeline; pipeline('text-classification', model='${EMOTION_MODEL}', top_k=None)"

EXPOSE 8000
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
