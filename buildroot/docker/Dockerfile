FROM debian:stable-slim

SHELL ["/bin/bash", "-c"]

# Install necessary packages and Bash shell
RUN apt update
RUN apt install -y build-essential libssl-dev curl wget cmake pkg-config
# Install lts node
# RUN apt install -y nodejs npm
# cleanup
RUN apt clean

# Install riji
RUN wget -O /usr/local/bin/riji https://github.com/fire-lib/riji/releases/latest/download/riji
RUN chmod +x /usr/local/bin/riji

# Now copy the necessary files



# COPY --chown=build . .

# RUN adduser --disabled-password --shell /bin/bash build
# USER build
# WORKDIR /home/build

# # Install latest Rust
# RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# ENV PATH="/home/build/.cargo/bin:${PATH}"

# RUN node --version && npm --version && cargo --version && riji --version

CMD ["/bin/bash"]