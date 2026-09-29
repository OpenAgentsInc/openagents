FROM python:3.12-slim
WORKDIR /app
COPY backend /app
RUN pip install .
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0"]
