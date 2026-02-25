error() {
    echo "$@" >&2
}

fatal() {
    error "$@"
    exit 1
}

set_source_and_root_dir() {
    { set +x; } 2>/dev/null
    source_dir="$( cd -P "$( dirname "$0" )" >/dev/null 2>&1 && pwd )"
    root_dir=$(cd $source_dir && cd ../ && pwd)
    cd $root_dir
}
