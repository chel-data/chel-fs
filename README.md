# Chel-FS

## Introduction

Chel-FS is a high performance filesystem built on top of DAOS, a mordern flash medium first storage pool. Chel-FS is aimed for workloads that require high performance filesystem operations. Chel-FS is a layer that is built on top of DAOS. Only filesystem metadata operations will go through Chel-FS, user data will directly flow between DAOS servers and clients. In such a way we can have both high IO bandwidth and IOPS.
