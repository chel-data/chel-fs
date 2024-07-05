
# Generate DOAS API Binding


## Steps

We use bindgen cli to generate API binding for DAOS. We need a Rust environment with DAOS library and header files installed. It is suggested to use a Rocky 8.9 Linux.

1.  Install DAOS cluster.
    Refer to docs/qemu-vms.md for how to install DAOS cluster.
2.  Install Rust environment on daos-client.
    Refer to this [page](https://www.rust-lang.org/tools/install) for Rust installation. It is quite easy just run following command.
    
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        source ~/.cargo/env
3.  Install bindgen tool using `cargo`.
    
        cargo install bindgen-cli
4.  Run bindgen to generate binding for daos\*.h files. It uses `--alowlist-file` to whitelist the daos\*.h header files. It prevents bindgen generate binding for system header files.
    
        ~/.cargo/bin/bindgen --allowlist-file /usr/include/daos_api.h --allowlist-file /usr/include/daos_fs.h --allowlist-file /usr/include/daos_obj_class.h --allowlist-file /usr/include/daos_security.h --allowlist-file /usr/include/daos_array.h --allowlist-file /usr/include/daos_fs_sys.h --allowlist-file /usr/include/daos_obj.h --allowlist-file /usr/include/daos_task.h --allowlist-file /usr/include/daos_cont.h --allowlist-file /usr/include/daos.h --allowlist-file /usr/include/daos_pool.h --allowlist-file /usr/include/daos_types.h --allowlist-file /usr/include/daos_errno.h --allowlist-file /usr/include/daos_kv.h --allowlist-file /usr/include/daos_prop.h --allowlist-file /usr/include/daos_uns.h --allowlist-file /usr/include/daos_event.h --allowlist-file /usr/include/daos_mgmt.h --allowlist-file /usr/include/daos_s3.h --allowlist-file /usr/include/daos_version.h  /usr/include/daos.h -o daos.rs

