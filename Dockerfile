FROM ubuntu:latest

RUN mkdir -p /app
WORKDIR /app
COPY . /app

RUN add-apt-repository ppa:ondrej/php \
&&apt install -y php8.2 php8.2-dev php8.2-cli php8.2-{bz2,curl,mbstring,intl,zip,bcmath,gmp,simplexml,xml,dom,sqlite3}\
&&apt install -y composer  php-pear  \
&&pecl channel-update pecl.php.net \
&&pecl install grpc-1.57.0 protobuf \
&&composer install 

RUN curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - &&\
apt-get install -y nodejs &&\
npm i

RUN echo "extension=grpc.so" >> /etc/php/8.2/cli/php.ini
RUN echo "(npm run dev&)&&php artisan migrate -n&&php artisan serve" > /init.sh

EXPOSE 8000

CMD ["bash", "/init.sh"]