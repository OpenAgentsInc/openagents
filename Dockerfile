# Use the official PostgreSQL image as the base image
FROM postgres:15

# Install CA certificates, and dependencies required for building pgvector
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates \
       build-essential \
       postgresql-server-dev-15 \
       git \
    # Clone and build pgvector
    && git clone https://github.com/ankane/pgvector.git \
    && cd pgvector \
    && make \
    && make install \
    # Clean up unnecessary packages and files to reduce image size
    && apt-get purge -y --auto-remove build-essential postgresql-server-dev-15 git \
    && rm -rf /var/lib/apt/lists/* /pgvector

# Set the default command for the container. This is what runs when the container starts.
CMD ["postgres"]
