#include "lang.h"


// 全局语言指针（由 main 设置）
static int g_lang = 0;

void SetLanguage(int lang) { g_lang = lang; }
int  GetLanguage()         { return g_lang; }

// 字符串表: key → { CN, EN, JP, KO }
static const wchar_t* g_strings[][4] = {
    // 0
    {L"KeyState 设置", L"KeyState Settings", L"KeyState 設定", L"KeyState 설정"},
    // 1
    {L"基本设置", L"Basic Settings", L"基本設定", L"기본 설정"},
    // 2
    {L"显示总数", L"Show Total", L"総数表示", L"키별 횟수"},
    // 3
    {L"显示KPS", L"Show KPS", L"KPS表示", L"KPS 표시"},
    // 4
    {L"显示汇总", L"Show Summary", L"集計表示", L"합계 표시"},
    // 5
    {L"显示轨道", L"Show History", L"軌道表示", L"트랙 표시"},
    // 6
    {L"显示BPM", L"Show BPM", L"BPM表示", L"BPM 표시"},
    // 7
    {L"固定", L"Click Through", L"クリックスルー", L"클릭 통과"},
    // 8
    {L"按键大小 (px)", L"Key Size (px)", L"キーサイズ (px)", L"키 크기 (px)"},
    // 9
    {L"按键间距 (px)", L"Key Spacing (px)", L"キー間隔 (px)", L"키 간격 (px)"},
    // 10
    {L"轨道高度 (px)", L"Track Height (px)", L"軌道高さ (px)", L"트랙 높이 (px)"},
    // 11
    {L"增长速率 (px/s)", L"Grow Speed (px/s)", L"成長速度 (px/s)", L"막대 성장 속도 (px/s)"},
    // 12
    {L"上浮速率 (px/s)", L"Float Speed (px/s)", L"浮上速度 (px/s)", L"막대 상승 속도 (px/s)"},
    // 13
    {L"方块上限 (%轨道)", L"Block Max (%track)", L"ブロック上限 (%軌道)", L"막대 최대 높이 (%)"},
    // 14
    {L"渲染帧率", L"Render FPS", L"描画FPS", L"화면 갱신 FPS"},
    // 15
    {L"按键映射", L"Key Mappings", L"キーマッピング", L"키 매핑"},
    // 16
    {L"添加按键", L"Add Key", L"キー追加", L"키 추가"},
    // 17
    {L"删除按键", L"Delete Key", L"キー削除", L"키 삭제"},
    // 18
    {L"外观设置 (请先在列表中选中一个按键)", L"Appearance (select a key from list)", L"外観設定 (リストから選択)", L"외관 설정 (목록에서 키 선택)"},
    // 19
    {L"字体颜色", L"Font Color", L"フォント色", L"글자색"},
    // 20
    {L"正常底色", L"Normal BG", L"通常背景", L"기본 배경"},
    // 21
    {L"按下底色", L"Press BG", L"押下背景", L"누름 배경"},
    // 22
    {L"保存并应用", L"Save & Apply", L"保存＆適用", L"저장 및 적용"},
    // 23
    {L"重置计数", L"Reset Totals", L"カウントリセット", L"횟수 초기화"},
    // 24
    {L"语言", L"Language", L"言語", L"언어"},
    // 25
    {L"中文", L"中文", L"中文", L"中文"},
    // 26
    {L"English", L"English", L"English", L"English"},
    // 27
    {L"日本語", L"日本語", L"日本語", L"日本語"},
    // 28
    {L"按任意键添加… (ESC 取消)", L"Press any key to add… (ESC to cancel)", L"キーを押して追加… (ESC取消)", L"추가할 키를 누르세요 (Esc로 취소)"},
    // 29
    {L"该按键已在列表中。", L"This key is already in the list.", L"このキーは既に存在します。", L"이미 등록된 키입니다."},
    // 30
    {L"重复按键", L"Duplicate Key", L"重複キー", L"중복된 키"},
    // 31
    {L"请先选中要删除的按键。", L"Please select a key to delete.", L"削除するキーを選択してください。", L"삭제할 키를 선택하세요."},
    // 32
    {L"未选择", L"No Selection", L"未選択", L"선택한 키 없음"},
    // 33
    {L"至少保留一个按键映射。", L"At least one key must remain.", L"最低1つのキーが必要です。", L"키를 하나 이상 남겨야 합니다."},
    // 34
    {L"已达上限", L"Limit", L"上限", L"삭제 제한"},
    // 35
    {L"所有按键计数已清零。", L"All totals have been reset to 0.", L"カウントをリセットしました。", L"모든 키의 누적 횟수를 0으로 초기화했습니다."},
    // 36
    {L"重置完成", L"Reset", L"リセット", L"횟수 초기화"},
    // 37
    {L"8分",    L"8th",    L"8分", L"8분음표"},
    // 38
    {L"16分",   L"16th",   L"16分", L"16분음표"},
    // 39
    {L"32分",   L"32nd",   L"32分", L"32분음표"},
    // 40
    {L"64分",   L"64th",   L"64分", L"64분음표"},
    // 41
    {L"轨道间距 (px)",     L"Track Gap (px)",     L"トラック間隔 (px)", L"트랙 간격 (px)"},
    // 42
    {L"轨道透明度",        L"Track Alpha",        L"トラック透明度", L"트랙 불투명도"},
    // 43
    {L"方块透明度",        L"Block Alpha",        L"ブロック透明度", L"막대 불투명도"},
    // 44
    {L"BPM 合并 (ms)",    L"BPM Merge (ms)",     L"BPM統合 (ms)", L"동시 입력 간격 (ms)"},
    // 45
    {L"\U0001F3A8 主题",  L"\U0001F3A8 Theme",   L"\U0001F3A8 テーマ", L"\U0001F3A8 테마"},
    // 46
    {L"自定义",           L"Custom",             L"カスタム", L"사용자 지정"},
    // 47
    {L"预设1",            L"Preset1",            L"プリセット1", L"프리셋 1"},
    // 48
    {L"预设2",            L"Preset2",            L"プリセット2", L"프리셋 2"},
    // 49
    {L"预设3",            L"Preset3",            L"プリセット3", L"프리셋 3"},
    // 50
    {L"盒子颜色",         L"Box Colors",         L"ボックス色", L"통계 상자 색상"},
    // 51
    {L"BG",               L"BG",                 L"BG", L"배경"},
    // 52
    {L"字体",             L"Font",               L"フォント", L"글꼴"},
    // 53
    {L"边框宽度 (px)",    L"Border W (px)",      L"境界幅 (px)", L"테두리 두께 (px)"},
    // 54
    {L"透明度 (%)",       L"Opacity (%)",        L"不透明度 (%)", L"불투명도 (%)"},
    // 55
    {L"重置默认设置",     L"Reset Defaults",     L"デフォルトに戻す", L"기본 설정으로 복원"},
    // 56
    {L"确认将所有设置重置为默认值？\n此操作无法撤销。", L"Reset ALL settings to defaults?\nThis cannot be undone.", L"全ての設定をデフォルトに戻しますか？\nこの操作は取り消せません。", L"모든 설정을 기본값으로 되돌릴까요?\n이 작업은 취소할 수 없습니다."},
    // 57
    {L"确认重置",         L"Confirm Reset",      L"リセット確認", L"복원 확인"},
    // 58
    {L"KPS 图表",         L"KPS Chart",          L"KPSチャート", L"KPS 그래프"},
    // 59
    {L"显示图表",         L"Show Chart",         L"チャート表示", L"그래프 표시"},
    // 60
    {L"时间范围 (s)",     L"Time Range (s)",     L"時間範囲 (s)", L"표시 범위 (초)"},
    // 61
    {L"背景色",           L"Chart BG",           L"背景色", L"배경색"},
    // 62
    {L"折线色",           L"Chart Line",         L"ライン色", L"선 색상"},
    // 63
    {L"圆角",             L"Radius",             L"角丸", L"모서리 둥글기"},
    // 64
    {L"图表宽度",    L"Chart W",       L"チャート幅", L"그래프 너비"},
    // 65
    {L"图表高度",    L"Chart H",       L"チャート高さ", L"그래프 높이"},
    // 66
    {L"左边距",           L"Margin L",           L"左余白", L"왼쪽 여백"},
    // 67
    {L"右边距",           L"Margin R",           L"右余白", L"오른쪽 여백"},
    // 68
    {L"上边距",           L"Margin T",           L"上余白", L"위쪽 여백"},
    // 69
    {L"下边距",           L"Margin B",           L"下余白", L"아래쪽 여백"},
    // 70
    {L"预设4",            L"Preset4",            L"プリセット4", L"프리셋 4"},
    // 71
    {L"预设5",            L"Preset5",            L"プリセット5", L"프리셋 5"},
    // 72
    {L"预设6",            L"Preset6",            L"プリセット6", L"프리셋 6"},
    // 73
    {L"预设7",            L"Preset7",            L"プリセット7", L"프리셋 7"},
    // 74
    {L"预设8",            L"Preset8",            L"プリセット8", L"프리셋 8"},
    // 75
    {L"预设9",            L"Preset9",            L"プリセット9", L"프리셋 9"},
    // 76
    {L"预设10",           L"Preset10",           L"プリセット10", L"프리셋 10"},
    // 77
    {L"预设11",           L"Preset11",           L"プリセット11", L"프리셋 11"},
    // 78
    {L"预设12",           L"Preset12",           L"プリセット12", L"프리셋 12"},
    // 79
    {L"预设13",           L"Preset13",           L"プリセット13", L"프리셋 13"},
    // 80
    {L"预设14",           L"Preset14",           L"プリセット14", L"프리셋 14"},
    // 81
    {L"预设15",           L"Preset15",           L"プリセット15", L"프리셋 15"},
    // 82
    {L"配色编辑器",       L"Theme Editor",       L"テーマエディタ", L"테마 편집기"},
    // 83
    {L"显示",             L"Display",            L"表示", L"표시"},
    // 84
    {L"轨道",             L"Track",             L"トラック", L"트랙"},
    // 85
    {L"主题",             L"Theme",              L"テーマ", L"테마"},
    // 86
    {L"图表",             L"Chart",              L"チャート", L"그래프"},
    // 87
    {L"按键",             L"Keys",               L"キー", L"키"},
    // 88
    {L"置顶显示",         L"Always On Top",      L"最前面に表示", L"항상 위에 표시"},
    // 89
    {L"显示轨道线",       L"Show Track Lines",   L"トラック線表示", L"트랙 선 표시"},
    // 90
    {L"显示网格线",       L"Show Grid Lines",    L"グリッド線表示", L"격자선 표시"},
    // 91
    {L"折线图", L"Line", L"折れ線", L"꺾은선"},
    // 92
    {L"散点图", L"Scatter", L"散布図", L"산점도"},
    // 93
    {L"柱状图", L"Bar", L"棒グラフ", L"막대"},
    // 94
    {L"渐变填充", L"Gradient Fill", L"グラデーション", L"그라데이션"},
    // 95
    {L"布局模式", L"Layout Mode", L"レイアウトモード", L"배치 모드"},
    // 96
    {L"常规模式", L"Normal Mode", L"通常モード", L"일반 배치"},
    // 97
    {L"自由模式", L"Free Mode", L"フリーモード", L"자유 배치"},
    // 98
    {L"录制功能", L"Recording", L"録画機能", L"입력 기록"},
    // 99
    {L"录制快捷键", L"Record Hotkey", L"録画ショートカット", L"기록 단축키"},
    // 100
    {L"点击设置录制快捷键…", L"Click to set record hotkey…", L"クリックして録画キー設定…", L"클릭하여 단축키 설정"},
    // 101
    {L"按任意键设置录制快捷键… (ESC 取消)", L"Press any key for record hotkey… (ESC cancel)", L"録画キーを押してください… (ESC取消)", L"단축키를 누르세요 (Esc로 취소)"},
    // 102
    {L"录制中… 再次按下快捷键停止", L"Recording… Press hotkey again to stop", L"録画中… もう一度押して停止", L"기록 중 · 단축키를 다시 누르면 종료"},
    // 103
    {L"录制已保存", L"Recording saved", L"録画保存完了", L"기록 저장 완료"},
    // 104
    {L"录制数据已保存到", L"Recording data saved to", L"録画データを保存しました", L"입력 기록을 다음 위치에 저장했습니다."},
    // 105
    {L"未录制", L"Not Recording", L"未録画", L"기록 대기 중"},
    // 106
    {L"自定义宽度 (0=默认)", L"Custom Width (0=default)", L"カスタム幅 (0=デフォルト)", L"사용자 지정 너비 (0은 기본값)"},
    // 107
    {L"自定义高度 (0=默认)", L"Custom Height (0=default)", L"カスタム高さ (0=デフォルト)", L"사용자 지정 높이 (0은 기본값)"},
    // 108
    {L"网格吸附", L"Grid Snap", L"グリッドスナップ", L"격자에 맞춤"},
    // 109
    {L"网格大小 (px)", L"Grid Size (px)", L"グリッドサイズ (px)", L"격자 크기 (px)"},
    // 110
    {L"宽:", L"W:", L"幅:", L"너비"},
    // 111
    {L"高:", L"H:", L"高:", L"높이"},
    // 112
    {L"宽 (0=默认)", L"W (0=default)", L"幅 (0=標準)", L"너비 (0은 기본값)"},
    // 113
    {L"高 (0=默认)", L"H (0=default)", L"高さ (0=標準)", L"높이 (0은 기본값)"},
    // 114
    {L"上升式", L"Rising", L"上昇式", L"위로 상승"},
    // 115
    {L"下落式", L"Falling", L"落下式", L"아래로 하강"},
    // 116
    {L"轨道尺寸", L"Track Size", L"軌道サイズ", L"트랙 크기"},
    // 117
    {L"方块动画", L"Block Animation", L"ブロックアニメーション", L"막대 애니메이션"},
    // 118
    {L"轨道背景透明度", L"Track BG Alpha", L"トラック背景透明度", L"트랙 배경 투명도"},
    // 119
    {L"显示区域边界", L"Show Boundary", L"境界表示", L"영역 테두리 표시"},
    // 120
    {L"主题预设", L"Theme Presets", L"テーマプリセット", L"테마 프리셋"},
    // 121
    {L"背景色", L"BG Color", L"背景色", L"배경색"},
    // 122
    {L"字体色", L"Font Color", L"フォント色", L"글자색"},
    // 123
    {L"线条色", L"Line Color", L"ライン色", L"선 색상"},
    // 124
    {L"宽", L"W", L"幅", L"너비"},
    // 125
    {L"高", L"H", L"高さ", L"높이"},
    // 126
    {L"图表类型", L"Chart Type", L"チャートタイプ", L"그래프 종류"},
    // 127
    {L"边距", L"Margins", L"余白", L"여백"},
    // 128
    {L"颜色", L"Colors", L"色", L"색상"},
    // 129
    {L"外观", L"Appearance", L"外観", L"외관"},
    // 130
    {L"吸附", L"Snap", L"スナップ", L"고정"},
    // 131
    {L"吸附到按键映射下方", L"Snap below keys", L"キー下にスナップ", L"키 표시 아래에 붙이기"},
    // 132
    {L"确定", L"OK", L"OK", L"확인"},
    // 133
    {L"取消", L"Cancel", L"キャンセル", L"취소"},
    // 134
    {L"重置确认", L"Reset Confirm", L"リセット確認", L"복원 확인"},
    // 135
    {L"捕获按键", L"Capture Key", L"キーキャプチャ", L"키 추가"},
    // 136
    {L"捕获录制快捷键", L"Capture Record Key", L"録画キーキャプチャ", L"기록 단축키 설정"},
    // 137
    {L"名称", L"Name", L"名前", L"이름"},
    // 138
    {L"音符划分", L"Note Division", L"音符分割", L"음표 단위"},
    // 139
    {L"当前字体: %s", L"Current Font: %s", L"現在のフォント: %s", L"현재 글꼴: %s"},
    // 140
    {L"选择字体...", L"Choose Font...", L"フォント選択...", L"글꼴 선택…"},
    // 141
    {L"轨道方向", L"Track Direction", L"軌道方向", L"트랙 방향"},
    // 142
    {L"自由模式", L"Free Mode", L"フリーモード", L"자유 배치"},
    // 143
    {L"布局", L"Layout", L"レイアウト", L"배치"},
    // 144
    {L"尺寸", L"Size", L"サイズ", L"크기"},
    // 145
    {L"窗口", L"Window", L"ウィンドウ", L"창"},
    // 146
    {L"字体", L"Font", L"フォント", L"글꼴"},
    // 147
    {L"透明度", L"Alpha", L"透明度", L"투명도"},
    // 148
    {L"按下设置的快捷键开始/停止录制。", L"Press hotkey to start/stop recording.", L"ショートカットキーで録画開始/停止。", L"단축키를 누르면 기록을 시작하거나 중지합니다."},
    // 149
    {L"点击设置录制快捷键…", L"Click to set record hotkey…", L"クリックして録画キー設定…", L"클릭하여 기록 단축키 설정"},
    // 150
    {L"边框颜色", L"Border Color", L"枠線色", L"테두리 색상"},
    // 151
    {L"快捷键设置", L"Hotkey Settings", L"ショートカット設定", L"단축키 설정"},
    // 152
    {L"UI 主题", L"UI Theme", L"UIテーマ", L"UI 테마"},
    // 153
    {L"红", L"Red", L"赤", L"빨강"},
    // 154
    {L"橙", L"Orange", L"橙", L"주황"},
    // 155
    {L"黄", L"Yellow", L"黄", L"노랑"},
    // 156
    {L"绿", L"Green", L"緑", L"초록"},
    // 157
    {L"蓝", L"Blue", L"青", L"파랑"},
    // 158
    {L"靛", L"Indigo", L"藍", L"남색"},
    // 159
    {L"紫", L"Purple", L"紫", L"보라"},
    // 160
    {L"● 未录制", L"● Idle", L"● 未録画", L"● 기록 대기 중"},
    // 161
    {L"● 录制中", L"● Recording", L"● 録画中", L"● 기록 중"},
    // 162
    {L"输出目录", L"Output Dir", L"出力先", L"저장 폴더"},
    // 163
    {L"选择目录...", L"Browse...", L"選択...", L"폴더 선택…"},
    // 164
    {L"轨道:", L"Track:", L"軌道:", L"트랙"},
    // 165
    {L"上浮/下沉速率 (px/s)", L"Rise/Sink Speed (px/s)", L"上昇/下降速度 (px/s)", L"막대 이동 속도 (px/s)"},
    // 166
    {L"区域宽度 (px)", L"Area Width (px)", L"領域幅 (px)", L"영역 너비 (px)"},
    // 167
    {L"区域高度 (px)", L"Area Height (px)", L"領域高さ (px)", L"영역 높이 (px)"},
    // 168
    {L"宽度 (px)", L"Width (px)", L"幅 (px)", L"너비 (px)"},
    // 169
    {L"高度 (px)", L"Height (px)", L"高さ (px)", L"높이 (px)"},
    // 170
    {L"左边距 (px)", L"Margin L (px)", L"左余白 (px)", L"왼쪽 여백 (px)"},
    // 171
    {L"右边距 (px)", L"Margin R (px)", L"右余白 (px)", L"오른쪽 여백 (px)"},
    // 172
    {L"上边距 (px)", L"Margin T (px)", L"上余白 (px)", L"위쪽 여백 (px)"},
    // 173
    {L"下边距 (px)", L"Margin B (px)", L"下余白 (px)", L"아래쪽 여백 (px)"},
    // 174
    {L"圆角 (px)", L"Radius (px)", L"角丸 (px)", L"모서리 둥글기 (px)"},
    // 175
    {L"X 偏移 (px)", L"X Offset (px)", L"X オフセット (px)", L"가로 오프셋 (px)"},
    // 176
    {L"Y 偏移 (px)", L"Y Offset (px)", L"Y オフセット (px)", L"세로 오프셋 (px)"},
    // 177
    {L"重置默认", L"Reset Default", L"デフォルトに戻す", L"기본값 복원"},
    // 178
    {L"设置面板", L"Settings Panel", L"設定パネル", L"설정 열기"},
    // 179
    {L"主题编辑器", L"Theme Editor", L"テーマエディタ", L"테마 편집기"},
    // 180
    {L"切换按键映射", L"Toggle Key Display", L"キー表示切替", L"키 표시 전환"},
    // 181
    {L"下一个主题", L"Next Theme", L"次のテーマ", L"다음 테마"},
    // 182
    {L"上一个主题", L"Prev Theme", L"前のテーマ", L"이전 테마"},
    // 183
    {L"切换轨道", L"Toggle Track", L"トラック切替", L"트랙 표시 전환"},
    // 184
    {L"切换图表", L"Toggle Chart", L"チャート切替", L"그래프 표시 전환"},
    // 185
    {L"所有快捷键均使用 Ctrl+Shift+[按键] 组合。点击按钮后按下新按键即可修改。", L"All hotkeys use Ctrl+Shift+[key]. Click button then press a new key.", L"全てのショートカットは Ctrl+Shift+[キー]。ボタン→新キー押下で変更。", L"단축키는 모두 Ctrl+Shift+[키] 조합입니다. 버튼을 클릭한 뒤 새 키를 누르면 바뀝니다."},
    // 186
    {L"按任意键… (ESC取消)", L"Press any key… (ESC cancel)", L"キーを押してください… (ESC取消)", L"키를 누르세요 (Esc로 취소)"},
    // 187
    {L"编辑: ", L"Edit: ", L"編集: ", L"편집: "},
    // 188
    {L"图表背景", L"Chart BG", L"チャート背景", L"그래프 배경"},
    // 189
    {L"图表线条", L"Chart Line", L"チャート線", L"그래프 선"},
    {L"한국어", L"한국어", L"한국어", L"한국어"}, // Korean
    {L"设置", L"Settings", L"設定", L"설정"}, // TraySettings
    {L"退出", L"Exit", L"終了", L"종료"}, // TrayExit
    {L"总计", L"Total", L"合計", L"합계"}, // Total
    {L"未设置", L"Not set", L"未設定", L"설정 안 함"}, // NotSet
    {L"选择录制文件输出目录", L"Select the folder for recording data", L"録画の保存先を選択", L"입력 기록을 저장할 폴더 선택"}, // RecordingFolder
    {L"Direct3DCreate9 失败！", L"Could not initialize Direct3D 9.", L"Direct3D 9 を初期化できませんでした。", L"Direct3D 9를 초기화할 수 없습니다."}, // CreateDirect3DFailed
    {L"创建 D3D9 设备失败！", L"Could not create the Direct3D 9 device.", L"Direct3D 9 デバイスを作成できませんでした。", L"Direct3D 9 장치를 만들 수 없습니다."}, // CreateDeviceFailed
    {L"错误", L"Error", L"エラー", L"오류"}, // Error
};

static_assert(sizeof(g_strings) / sizeof(g_strings[0]) == TXT_Error + 1);

const wchar_t* LANG(int key) {
    if (key < 0 || key >= (int)(sizeof(g_strings) / sizeof(g_strings[0]))) return L"";
    int idx = g_lang;
    if (idx < 0 || idx > static_cast<int>(Lang::KO)) idx = 0;
    return g_strings[key][idx];
}

const wchar_t* ThemeDisplayName(int index, const std::wstring& name) {
    const int key = index <= 3 ? 46 + index : 70 + index - 4;
    return name.empty() || name == g_strings[key][static_cast<int>(Lang::CN)]
        ? LANG(key) : name.c_str();
}
