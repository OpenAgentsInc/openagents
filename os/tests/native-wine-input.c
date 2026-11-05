// Firm reason: 2 (independent Win32 SDK experiment: compare addressed messages and input-stream keys without requesting foreground activation).
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

static HWND target;
static int matches;
static int notify(UINT message, WPARAM w, LPARAM l) {
  DWORD_PTR result=0;
  return SendMessageTimeoutW(target,message,w,l,SMTO_ABORTIFHUNG|SMTO_BLOCK,100,&result)!=0;
}
static BOOL CALLBACK find_game(HWND w, LPARAM unused) {
  (void)unused;
  wchar_t title[128], path[1024]; DWORD pid, size=1024;
  if (!GetWindowTextW(w,title,128) || wcscmp(title,L"World of Warcraft")) return TRUE;
  GetWindowThreadProcessId(w,&pid);
  HANDLE process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,FALSE,pid);
  if (!process) return TRUE;
  BOOL ok=QueryFullProcessImageNameW(process,0,path,&size); CloseHandle(process);
  const wchar_t *base=ok?wcsrchr(path,L'\\'):NULL;
  if (base && !_wcsicmp(base+1,L"Wow.exe") && IsWindowVisible(w)) {target=w;matches++;}
  return TRUE;
}
int main(int argc,char **argv) {
  if (argc<2) return 2;
  int tap=!strcmp(argv[1],"tap"),click=!strcmp(argv[1],"click");
  if (tap) {
    if(argc!=4 || strlen(argv[2])!=1 || !strchr("WASDLE",argv[2][0]) || atoi(argv[3])<1 || atoi(argv[3])>500) return 2;
  } else if(click) {if(argc!=4) return 2;}
  else if (argc!=2 || (strcmp(argv[1],"inspect") && strcmp(argv[1],"post") && strcmp(argv[1],"notify") && strcmp(argv[1],"send"))) return 2;
  EnumWindows(find_game,0);
  if(matches!=1) {fprintf(stderr,"Expected one visible WoW window; found %d\n",matches);return 3;}
  DWORD pid; GetWindowThreadProcessId(target,&pid);
  HWND before=GetForegroundWindow(); UINT down=0,up=0;
  if(click) {
    RECT bounds;int x=atoi(argv[2]),y=atoi(argv[3]);
    if(!GetClientRect(target,&bounds) || x<0 || y<0 || x>=bounds.right || y>=bounds.bottom) return 2;
    LPARAM at=MAKELPARAM(x,y);
    PostMessageW(target,WM_MOUSEMOVE,0,at);
    down=PostMessageW(target,WM_LBUTTONDOWN,MK_LBUTTON,at);Sleep(30);
    up=PostMessageW(target,WM_LBUTTONUP,0,at);
  } else if(tap) {
    UINT key=(unsigned char)argv[2][0];LPARAM scan=(LPARAM)MapVirtualKeyW(key,MAPVK_VK_TO_VSC)<<16;
    down=PostMessageW(target,WM_KEYDOWN,key,1|scan);Sleep((DWORD)atoi(argv[3]));
    up=PostMessageW(target,WM_KEYUP,key,1|scan|((LPARAM)3<<30));
  } else if (!strcmp(argv[1],"notify")) {
    if(before==target) return 4;
    int active=notify(WM_ACTIVATEAPP,TRUE,0) && notify(WM_ACTIVATE,WA_ACTIVE,0) && notify(WM_SETFOCUS,0,0);
    LPARAM scan=(LPARAM)MapVirtualKeyW('L',MAPVK_VK_TO_VSC)<<16;
    if(active) {down=notify(WM_KEYDOWN,'L',1|scan);Sleep(30);up=notify(WM_KEYUP,'L',1|scan|((LPARAM)3<<30));}
    notify(WM_KILLFOCUS,0,0);notify(WM_ACTIVATE,WA_INACTIVE,0);notify(WM_ACTIVATEAPP,FALSE,0);
  } else if (!strcmp(argv[1],"post")) {
    LPARAM scan=(LPARAM)MapVirtualKeyW('L',MAPVK_VK_TO_VSC)<<16;
    down=PostMessageW(target,WM_KEYDOWN,'L',1|scan);
    Sleep(30);
    up=PostMessageW(target,WM_KEYUP,'L',1|scan|((LPARAM)3<<30));
  } else if (!strcmp(argv[1],"send")) {
    if(before!=target) {fprintf(stderr,"WoW is not the Wine foreground window; refusing global SendInput\n");return 4;}
    INPUT key={0}; key.type=INPUT_KEYBOARD;key.ki.wVk='L';
    down=SendInput(1,&key,sizeof key);Sleep(30);
    key.ki.dwFlags=KEYEVENTF_KEYUP;up=SendInput(1,&key,sizeof key);
  }
  Sleep(100);
  HWND after=GetForegroundWindow();
  printf("{\"mode\":\"%s\",\"hwnd\":%llu,\"wine_pid\":%lu,\"foreground_before\":%llu,\"foreground_after\":%llu,\"target_foreground\":%s,\"foreground_unchanged\":%s,\"down_sent\":%u,\"up_sent\":%u}\n",argv[1],(unsigned long long)(UINT_PTR)target,(unsigned long)pid,(unsigned long long)(UINT_PTR)before,(unsigned long long)(UINT_PTR)after,before==target?"true":"false",before==after?"true":"false",down,up);
  return before!=after?5:strcmp(argv[1],"inspect") && (!down || !up)?6:0;
}
