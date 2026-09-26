FROM python:3.13-slim
# Installs a package at build time, which needs the network while the
# verifier itself runs without it.
RUN pip install --no-cache-dir six==1.16.0
COPY . /tests/
RUN mkdir -p /app
