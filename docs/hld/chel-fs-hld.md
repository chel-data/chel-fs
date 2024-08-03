

## Table of Contents

- [Chel-Fs](#chel-fs)
- [Design Principles](#design-principles)
  - [Disaggregated Distributed File System Service](#disaggregated-distributed-file-system-service)
  - [Chel-Fs Metadata Service (MDS)](#chel-fs-metadata-service-mds)
  - [Chel-Fs Client](#chel-fs-client)
  - [DAOS and Chel-Fs entity relationship](#daos-and-chel-fs-entity-relationship)
  - [Chel-Fs Snapshots](#chel-fs-snapshots)
  - [Chel-Fs Quotas](#chel-fs-quotas)
  - [Chel-Helm](#chel-helm)
- [Chel-Fs Components](#chel-fs-components)
- [Metadata Structures(Proto)](#metadata-structuresproto)
- [HLD for File Operation](#hld-for-file-operation)

# Chel-Fs

Chel File System(Chel-FS), is a disaggregated software defined distributed file system (POSIX compliant), built on top of DAOS (https://github.com/daos-stack/daos). The main objective of Chel-FS is to provide scalable file system metadata management at the same time, keep the stock performance/scale of DAOS for file data operations. Chel-FS is aimed for workloads that require high performance and scalable file system operations.

# Design Principles

## Disaggregated Distributed File System Service
The main objective of Chel-FS is to provide a disaggregated Distributed File System Service over DAOS Containers/Pool.
DAOS is capable to handle high volumes of Parallel Data Operations. But for high volumes of small file metadata operations
there is a need that this compute responsibility is distributed among disjoint file system metadata compute entities. We call these file system metadata compute entities as Chel-Fs MDS.
## Chel-Fs Metadata Service (MDS)  
As defined above, Chel-Fs 
## Chel-Fs Client  
## DAOS and Chel-Fs entity relationship  
## Chel-Fs Snapshots  
## Chel-Fs Quotas
## Chel-Helm

# Chel-Fs Components

# Metadata Structures(Proto)

# HLD for File Operation

