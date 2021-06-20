FROM rust

# https://mengfung.com/2020/09/docker-apt-get-cleanup/
RUN apt-get update && apt-gwt install -y \
    python3 \
    python3-dev \
    python3-pip \
    fonts-comic-neue \
    && apt-get clean && rm -f /var/lib/apt/lists/*_*

RUN mkdir emoji_builder
WORKDIR /emoji_builder

COPY requirements.txt /emoji_builder/

RUN python3 -m pip install -r requirements.txt

ADD . /emoji_builder

RUN ./github_workflow_setup.sh && \
    cargo build --release && \
    mv target/release/emoji_builder /bin && \
    cargo clean


VOLUME /emoji

ENV CLI_ARGS

WORKDIR /emoji

CMD /emoji_builder/github_workflow_setup.sh && /bin/emoji_builder $CLI_ARGS
