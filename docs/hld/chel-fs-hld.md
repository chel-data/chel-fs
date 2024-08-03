

## Table of Contents

- [Chel-FS](#chel-fs)
- [Design Principles](#design-principles)
  - [Disaggregated Distributed File System Service](#disaggregated-distributed-file-system-service)
  - [Chel-FS Metadata Service (MDS)](#chel-fs-metadata-service-mds)
    - [Sharded Compute Responsibility](#sharded-compute-responsibility)
    - [Delegate Locks and Capabilities](#delegate-locks-and-capabilities)
    - [FS layout on DAOS containers](#fs-layout-on-daos-containers)
    - [Distributed Transaction Synchronization](#distributed-transaction-synchronization)
  - [Chel-FS Client](#chel-fs-client)
  - [DAOS and Chel-FS entity relationship](#daos-and-chel-fs-entity-relationship)
  - [Chel-FS Snapshots](#chel-fs-snapshots)
  - [Chel-FS Quotas](#chel-fs-quotas)
  - [Chel-Helm](#chel-helm)
- [Chel-FS Internal Components](#chel-fs-internal-components)
- [Metadata Structures(Proto)](#metadata-structuresproto)
- [HLD for File Operation](#hld-for-file-operation)

# Chel-FS

Chel File System(Chel-FS), is a disaggregated software defined distributed file system (POSIX compliant), built on top of DAOS (https://github.com/daos-stack/daos). The main objective of Chel-FS is to provide scalable file system metadata management at the same time, keep the stock performance/scale of DAOS for file data operations. Chel-FS is aimed for workloads that require high performance and scalable file system operations.

![Image](chel-fs-hl-block.drawio.png)

# Design Principles

## Disaggregated Distributed File System Service
The main objective of Chel-FS is to provide a disaggregated Distributed File System Service over DAOS Containers/Pool.
DAOS is capable to handle high volumes of Parallel Data Operations. But for high volumes of small file metadata operations
there is a need that this compute responsibility is distributed among disjoint file system metadata compute entities. We call these file system metadata compute entities as Chel-FS MDS.

## Chel-FS Metadata Service (MDS)  
As defined above, Chel-FS MDS (referred as MDS in the future) is responsible for following,
### Sharded Compute Responsibility
Chel-FS metadata is sharded amongst a N number of MDS. The sharding of metadata entities is done on some properties of the entity For example:
It would usual to shard inodes using the inode's primary parent inode. (Note: A inode can have multiple parents due to multiple hardlinks, though this is exception for directories, as directories will have only 1 parent inode). 
The sharding is dictated on the following factors,
1. Optimal Layout of Chel-FS metadata on DAOS Objects. 
2. Disjoint Metadata Operations(most of the times) i,e 1 Metadata operation would most of the time involve 1 MDS.
   1. Less number of network hops for 1 Metadata operation
   2. Less number OR None distributed transaction/s in a metadata operation 
3. Ease of Predictive Caching (Read-ahead caching)
4. Ease of rebuild/reload during a failover or failback of MDS.
5. 
(TODO : Details in LLD of MDS Sharding)

Chel-FS cluster could have multiple file systems created and all the metadata of all these filesystems would be sharded on N number of MDS of the cluster.

Since storage/DAOS is ubiquitous i.e accessed by all the MDS without having any storage affinity, The number of MDS in a cluster can be increased or decreased seamlessly, depending on Compute requirements of the workload.

### Delegate Locks and Capabilities

### FS layout on DAOS containers
### Distributed Transaction Synchronization

## Chel-FS Client

## DAOS and Chel-FS entity relationship  

## Chel-FS Snapshots  

## Chel-FS Quotas

## Chel-Helm

# Chel-FS Internal Components

# Metadata Structures(Proto)

# HLD for File Operation

