// Firm reason: 6 (the Vulkan loader's C ABI).
// vk-load.h's functions, from vulkan-1.dll (GitHub #13782).
#include "vk-load.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>

#define UI_VK_DEFINE(name) PFN_##name name;
UI_VK_FUNCTIONS(UI_VK_DEFINE)
#undef UI_VK_DEFINE

int ui_vk_load(char *why, size_t cap) {
  static int loaded;
  if (loaded) return 1;
  HMODULE dll = LoadLibraryW(L"vulkan-1.dll");
  if (!dll) {
    snprintf(why, cap, "OpenAgents draws with Vulkan, and this PC has no Vulkan driver (vulkan-1.dll). Install or update the graphics driver from the PC or graphics card maker, then open OpenAgents again.");
    return 0;
  }
#define UI_VK_GET(name) \
  name = (PFN_##name)(void (*)(void))GetProcAddress(dll, #name); \
  if (!name) { snprintf(why, cap, "The Vulkan driver on this PC is too old (it has no %s). Update the graphics driver, then open OpenAgents again.", #name); return 0; }
  UI_VK_FUNCTIONS(UI_VK_GET)
#undef UI_VK_GET
  loaded = 1;
  return 1;
}
