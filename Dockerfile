FROM rustlang/rust:nightly
# MAINTAINER billy

WORKDIR /var/www/microservice
COPY . .

RUN rustc --version
RUN cargo build

CMD ["microservice"]
