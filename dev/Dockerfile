FROM ubuntu:22.04

RUN mkdir -p /app && mkdir -p /openagents-pool
WORKDIR /app


ENV DEBIAN_FRONTEND=noninteractive
RUN echo "Etc/UTC" > /etc/timezone

RUN apt update \
&&apt install -y software-properties-common\
&&apt upgrade -y \
&&add-apt-repository ppa:ondrej/php \
&&apt update \
&&apt install -y php8.2 php8.2-dev php8.2-cli php8.2-bz2 php8.2-curl php8.2-mbstring php8.2-intl php8.2-zip php8.2-bcmath php8.2-gmp php8.2-simplexml php8.2-xml php8.2-dom php8.2-sqlite3\
&&apt install -y composer  php-pear zlib1g-dev  \
&&pecl channel-update pecl.php.net \
&&pecl install grpc-1.57.0 protobuf \
&&apt clean

RUN curl -fsSL https://deb.nodesource.com/setup_22.x |  bash - \
&&apt-get install -y nodejs \
&&apt clean


RUN apt update \
&&apt install -y lsb-release curl gpg \
&& curl -fsSL https://packages.redis.io/gpg | gpg --dearmor -o /usr/share/keyrings/redis-archive-keyring.gpg \
&& echo "deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb $(lsb_release -cs) main" | tee /etc/apt/sources.list.d/redis.list \
&& apt update \
&& apt install -y redis \
&& apt clean \
&& systemctl enable redis-server

ENV POOL_SNAPSHOT="master"
ENV POOL_REPO="https://github.com/OpenAgentsInc/openagents-pool.git"

RUN cd /openagents-pool \
&&apt update \
&&apt install -y git \
&&apt clean \
&&git clone $POOL_REPO . 
 
RUN echo "extension=grpc.so" >> /etc/php/8.2/cli/php.ini

ADD dev/init.sh /init.sh

EXPOSE 8000
EXPOSE 5173
EXPOSE 5000
CMD ["bash", "/init.sh"]