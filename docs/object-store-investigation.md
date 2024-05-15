
# Object Storage Requirements


## Aspects to Consider

1.  Object Storage Type
    1.  one-premises
    2.  off-premises
2.  Interfaces
    1.  S3/GCS/Azure Blob
    2.  OpenStack Swift
    3.  implementatio specific
        1.  DAOS
        2.  Redis
        3.  TiKV
3.  Network Protocol
    1.  HTTP,REST
    2.  TCP/IP
    3.  RDMA
    4.  RPC over X
4.  Required Capabilities
    1.  PUT/Get
    2.  Snapshot
    3.  Transaction as plus
    4.  ACL?
    5.  Encryption?


## Criteria

1.  Network Protocol
    1.  Less CPU consumption
    2.  Simple network stack
2.  Lightweight Metadata
    1.  Metadata can be skipped for some object
3.  Distributed Transaction
    1.  Good to have.
4.  Support Flags/Hints
    1.  Differentiate metadata from data.
    2.  Differentiate hot data from cold data.
    3.  Maybe support streams.


## Options

1.  **DAOS**
2.  RADOS

