FROM fedora:rawhide

RUN dnf -y install \
    gcc \
    gcc-c++ \
    pkg-config \
    make \
    git \
    curl \
    gtk4-devel \
    libadwaita-devel \
    gdk-pixbuf2-devel \
    dbus-devel \
    gstreamer1-devel \
    gstreamer1-plugins-base-devel && \
    dnf clean all

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release || true

COPY . .

RUN cargo build --release

CMD ["bash"]