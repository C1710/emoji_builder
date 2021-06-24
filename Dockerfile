FROM rust as builder

# https://mengfung.com/2020/09/docker-apt-get-cleanup/
RUN apt-get update && apt-get install -y \
    python3 \
    python3-dev \
    python3-pip \
    && apt-get clean && rm -f /var/lib/apt/lists/*_*

RUN mkdir emoji_builder
WORKDIR /emoji_builder

COPY requirements.txt /emoji_builder/

RUN python3 -m pip install -r requirements.txt

COPY . /emoji_builder

RUN chmod +x /emoji_builder/github_workflow_setup.sh && \
    /emoji_builder/github_workflow_setup.sh && \
    cargo install --path .

FROM python:3.7-slim

VOLUME /emoji

ENV CLI_ARGS=blobmoji

COPY requirements.txt .

RUN apt-get update && apt-get install -y \
    fonts-comic-neue \
    && apt-get clean && rm -f /var/lib/apt/lists/*_*  &&\
    python3 -m pip install -r requirements.txt

COPY --from=builder /usr/local/cargo/bin/emoji_builder /bin/emoji_builder
COPY github_workflow_setup.sh /github_workflow_setup.sh

WORKDIR /emoji

CMD /github_workflow_setup.sh && /bin/emoji_builder $CLI_ARGS
