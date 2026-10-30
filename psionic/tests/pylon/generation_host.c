/* Public generation-state persistence only; no signing-key handling. */
#define main pylon_host_main
#include "../../platform/pylon-host.c"
#undef main
#include <assert.h>
#include <sys/wait.h>
int coax_pylon_main(void) { return 0; }
static void path(const char *s) {
    assert(pyh_file_path_clear()==0);
    while (*s) assert(pyh_file_path_push((unsigned char)*s++)==0);
}
static void store(const char *s) {while (*s) assert(pyh_file_write_push((unsigned char)*s++)==0);}
int main(void) {
    char dir[]="/tmp/pylon-generation-test-XXXXXX";assert(mkdtemp(dir));
    char counter[1100],lock[1100],link[1100],fifo[1100],absent[1100];
    snprintf(counter,sizeof counter,"%s/counter",dir);snprintf(lock,sizeof lock,"%s/lock",dir);
    snprintf(link,sizeof link,"%s/link",dir);snprintf(fifo,sizeof fifo,"%s/fifo",dir);snprintf(absent,sizeof absent,"%s/absent/counter",dir);
    path(counter);assert(pyh_file_read()==-2);store("first\n");assert(pyh_file_write_commit()==0);
    path(counter);assert(pyh_file_read()==6&&pyh_file_byte(0)=='f');
    store("second\n");assert(pyh_file_write_commit()==0);assert(pyh_file_read()==7&&pyh_file_byte(0)=='s');
    struct stat st;assert(stat(counter,&st)==0&&(st.st_mode&077)==0);
    path(absent);store("not committed\n");assert(pyh_file_write_commit()==-1);
    path(counter);assert(pyh_file_read()==7&&pyh_file_byte(0)=='s');
    path(dir);assert(pyh_file_read()==-1); // present but unreadable as a file
    path(lock);assert(pyh_file_lock()==0);
    pid_t child=fork();assert(child>=0);
    if(child==0){close(g_state_lock_fd);g_state_lock_fd=-1;path(lock);_exit(pyh_file_lock()==-1?0:1);}
    int status;assert(waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==0);
    close(g_state_lock_fd);g_state_lock_fd=-1;
    path(lock);assert(pyh_file_lock()==0);close(g_state_lock_fd);g_state_lock_fd=-1;
    assert(chmod(lock,0644)==0);path(lock);assert(pyh_file_lock()==-1);assert(chmod(lock,0600)==0);
    assert(symlink(lock,link)==0);path(link);assert(pyh_file_lock()==-1);
    assert(mkfifo(fifo,0600)==0);path(fifo);assert(pyh_file_lock()==-1);
    unlink(fifo);unlink(link);unlink(lock);unlink(counter);assert(rmdir(dir)==0);
    return 0;
}
