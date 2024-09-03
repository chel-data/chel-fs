# Metadata Design


## Design Considerations

Chel-FS uses the metadata format in this document to store the information and relations of filesystem entities. It is one of the key designs in Chel-FS. Chel-FS utilizes the functionalities provided by DAOS to build directories and files. It takes insights from DFS of DAOS. We wish to maximize the metadata performance by co-locating dentries and inode structures. Because we reduce one level of indirection, we need to maintain back refs in regular files by storing back refs in the first chunk of regular file objects.


## Metadata Format

Chel-FS Metadata formats include formats for the superblock, directory structures and regular files. It is explained as follows.

-   Superblock
    Superblock is a hash table object. It store global filesystem parameters in this hash table. These parameters share the some dkey "chel\_fs\_sb". The parameters are stored in different akey's.
    -   chel\_fs\_magic
    -   chel\_fs\_version
-   Directory Structure
    Directories are DAOS objects with built-in hash tables. Every key-value entry is a directory entry. Keys are the encoded names of the directory entries with-in the parent directories. The keys are used as dkey's in the two level hash table. Akeys are fixed as zero. The values contain serialized inode structures. Unless one entry is a soft link, the value contains the path of the target.
    -   Inode Structure
        -   mode (POSIX permissions + entry type)
        -   oid (if this entry is a regular file)
        -   atime
        -   atime\_nano
        -   mtime
        -   mtime\_nano
        -   ctime
        -   ctime\_nano
        -   uid
        -   gid
        -   nlinks
        -   obj\_hlc
        -   chunk\_size
        -   total\_size
    -   Symbol Link
        -   target[256]
-   Regular File
    Regular files are byte array objects. The data of a regular file are divided into chunks. Chunks are stored with chunk offsets as dkey's. Regular files also store some attributes in chunk 0. This structure is defined as follows.
    
    -   Chunk 0
        -   chunk\_size
        -   nlinks
        -   links[&#x2026;][256]
    
    There are nlinks both in the inode structure in directory structures and chunk 0 of the target file. But the field in the inode structure is not accurately maintained due to performance consideration. It can only be used to judge if there is more than one link to this file other than determine how many links.
