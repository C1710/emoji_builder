FROM rust:1.44

RUN apt update && apt install -y \
    python3 \
    python3-dev \
    python3-pip \
    clang \
    fonts-comic-neue

RUN mkdir emoji_builder
WORKDIR /emoji_builder

COPY requirements.txt /emoji_builder/

RUN python3 -m pip install -r requirements.txt

ADD . /emoji_builder

# Not sure whether tests should happen in the container build process
# RUN LD_LIBRARY_PATH=$(echo /usr/lib/python3.*/config-3.*) cargo test
RUN LD_LIBRARY_PATH=$(echo /usr/lib/python3.*/config-3.*) cargo build --release

RUN cp target/release/emoji_builder /bin

CMD LD_LIBRARY_PATH=$(echo /usr/lib/python3.*/config-3.*) /bin/emoji_builder
