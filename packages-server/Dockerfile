## Build
FROM soerenmeier/fire-build

COPY --chown=build . ./packages-server
COPY --chown=build ./deps/packages-api ./packages-api

# now build the server
WORKDIR packages-server

RUN cargo b --release

WORKDIR ..

## release 1000
FROM soerenmeier/fire-release

COPY --chown=release --from=0 /home/build/packages-server/target/release/packages-server .

CMD ["./packages-server", "--config", "/data/config.toml"]

# CMD ["/bin/bash"]
