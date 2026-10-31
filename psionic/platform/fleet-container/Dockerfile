# Official Nix 2.24.9 amd64 builder, pinned by image digest. No APT TLS input.
FROM nixos/nix@sha256:f6f7814c2224dad18b14479fed5f0620be65d54625a4b97ae8b24b407e1b43a2 AS build
WORKDIR /src
COPY . .
RUN nix-build --option sandbox false --cores 2 --max-jobs 2 default.nix -A imageRoot --out-link /fleet-root && \
    nix-build --option sandbox false default.nix -A nativeTools --out-link /fleet-tools && \
    mkdir /runtime && cp -a /fleet-root/. /runtime/ && chmod 1777 /runtime/tmp
RUN /fleet-tools/bin/chroot --userspec=10001:10001 /runtime /bin/sh -c 'test -x /usr/local/bin/psionic-fleet && test -r /etc/passwd && test -r /etc/group && test -r /etc/nsswitch.conf && test -r /etc/ssl/certs/ca-certificates.crt && : > /tmp/fleet-build-write-check' && \
    rm /runtime/tmp/fleet-build-write-check && \
    /fleet-tools/bin/chroot /runtime /bin/sh -n /usr/local/bin/start-fleet
RUN /fleet-tools/bin/chroot /runtime /usr/local/bin/psionic-fleet --identity invalid > /fleet-native-usage-check.log 2>&1; test "$?" -eq 2
RUN cat /runtime/usr/share/psionic-fleet/NIX_TLS_PROVENANCE && \
    cat /runtime/usr/share/psionic-fleet/ELF_INTERPRETER && \
    cat /runtime/usr/share/psionic-fleet/ELF_DYNAMIC && \
    cat /runtime/usr/share/psionic-fleet/ELF_LDD && \
    cat /runtime/usr/share/psionic-fleet/RUNTIME_CLOSURE
FROM scratch
COPY --from=build /runtime/ /
USER 10001:10001
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENTRYPOINT ["/bin/sh", "/usr/local/bin/start-fleet"]
