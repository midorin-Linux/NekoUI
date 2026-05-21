# For Windows users. If you are linux or macOS user, please delete this line.
set shell := ["powershell.exe", "-c"]

help:
    just -l

#qdrant-setup:
#    docker pull qdrant/qdrant:latest
#    docker volume create qdrant_data
#
#qdrant-up:
#    $exists = docker ps -a --filter "name=^qdrant$" -q; if ($exists) { docker start qdrant } else { docker run -d --name qdrant -p 6333:6333 -p 6334:6334 -e QDRANT__SERVICE__GRPC_PORT="6334" -v qdrant_data:/qdrant/storage qdrant/qdrant:latest }
#
#qdrant-down:
#    $running = docker ps --filter "name=^qdrant$" -q; if ($running) { docker stop qdrant }
