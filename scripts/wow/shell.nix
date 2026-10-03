let
  pkgs = import (builtins.fetchTarball "https://github.com/NixOS/nixpkgs/archive/c25784012c9982bca5b3e0de87e90bbdac8927d3.tar.gz") {};
in pkgs.mkShell {
  hardeningDisable = [ "format" ];
  MYSQL_HOME = "${pkgs.libmysqlclient.dev}";
  packages = with pkgs; [ cmake ninja gcc git openssl zlib bzip2 mariadb libmysqlclient curl p7zip python3 ];
}
