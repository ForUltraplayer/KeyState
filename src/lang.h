#pragma once

#include <string>

// 语言体系：0=中文 1=English 2=日本語 3=한국어
enum class Lang { CN = 0, EN = 1, JP = 2, KO = 3 };

void SetLanguage(int lang);
int  GetLanguage();
const wchar_t* LANG(int key);

enum TextId {
    TXT_Korean = 190,
    TXT_TraySettings,
    TXT_TrayExit,
    TXT_Total,
    TXT_NotSet,
    TXT_RecordingFolder,
    TXT_CreateDirect3DFailed,
    TXT_CreateDeviceFailed,
    TXT_Error
};

const wchar_t* ThemeDisplayName(int index, const std::wstring& name);
