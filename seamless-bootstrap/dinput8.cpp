#include <windows.h>

#pragma comment(linker, "/export:DirectInput8Create=C:\\Windows\\System32\\dinput8.dll.DirectInput8Create")
#pragma comment(linker, "/export:DllCanUnloadNow=C:\\Windows\\System32\\dinput8.dll.DllCanUnloadNow,PRIVATE")
#pragma comment(linker, "/export:DllGetClassObject=C:\\Windows\\System32\\dinput8.dll.DllGetClassObject,PRIVATE")
#pragma comment(linker, "/export:DllRegisterServer=C:\\Windows\\System32\\dinput8.dll.DllRegisterServer,PRIVATE")
#pragma comment(linker, "/export:DllUnregisterServer=C:\\Windows\\System32\\dinput8.dll.DllUnregisterServer,PRIVATE")
#pragma comment(linker, "/export:GetdfDIJoystick=C:\\Windows\\System32\\dinput8.dll.GetdfDIJoystick")

BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID) {
    if (reason != DLL_PROCESS_ATTACH) return TRUE;
    DisableThreadLibraryCalls(module);
    wchar_t marker[8]{};
    if (GetEnvironmentVariableW(L"ERDTREE_TOWER_SEAMLESS_ENTRY", marker, 8) != 1 || marker[0] != L'1') return TRUE;
    wchar_t engine[32768]{};
    DWORD length = GetEnvironmentVariableW(L"ERDTREE_TOWER_MODENGINE", engine, 32768);
    if (!length || length >= 32768 || !LoadLibraryW(engine)) return FALSE;
    return TRUE;
}
