#include <ernativeui/ERNativeUI.hpp>
#include <Windows.h>
#include <array>
#include <atomic>
#include <filesystem>
#include "translations.h"

namespace {
HMODULE module{};
erui::Registration registration{};
std::atomic_bool ready{false};
std::atomic_bool english{false};
unsigned language_mode{};
const wchar_t* tr(const wchar_t* text) noexcept {
    if(english.load()) for(const auto& entry:tower_english_text) if(entry.first==text) return entry.second.data();
    return text;
}
std::array<std::uint8_t, 8> values{};
using Save = int (__cdecl*)(const std::uint8_t*);
Save save{};
using SaveBindings = int (__cdecl*)(const unsigned*);
using Activate = void (__cdecl*)(unsigned);
SaveBindings save_bindings{};
Activate activate{};
std::array<unsigned,5> key_codes{};
constexpr std::array<const char*,5> action_ids{"settle","overlay","archive","build","debug"};
std::array<unsigned,5> slots{0,1,2,3,4};
struct PreviewChoice {const wchar_t* label; const wchar_t* help; const wchar_t* confirm; const wchar_t* result; const wchar_t* alternate_result{};};
const std::array<PreviewChoice,3> spring_choices{{
    {L"汲取月辉　·　恢复生命", L"让泉水洗去伤痛。生命恢复至当前上限；不恢复专注、不补充圣杯瓶。每处泉水三选一。",
     L"汲取月辉\n\n生命将恢复至当前上限。\n选择后，本处泉水的其他馈赠将不可领取。\n\n演示确认流程？本页不会改变角色或本局进度。", L"生命恢复流程预览完成。\n\n正式效果：生命恢复至当前上限。\n本次仅预览，未恢复生命，也未消耗事件。"},
    {L"静听泉声　·　恢复专注", L"在月色中平息心神。专注恢复至当前上限；不恢复生命、不补充圣杯瓶。每处泉水三选一。",
     L"静听泉声\n\n专注将恢复至当前上限。\n选择后，本处泉水的其他馈赠将不可领取。\n\n演示确认流程？本页不会改变角色或本局进度。", L"专注恢复流程预览完成。\n\n正式效果：专注恢复至当前上限。\n本次仅预览，未恢复专注，也未消耗事件。"},
    {L"拾取遗落卢恩　·　800 卢恩", L"放弃休憩，拾起泉畔遗落的财富。获得 800 卢恩；不恢复生命或专注。每处泉水三选一。",
     L"拾取遗落卢恩\n\n将获得 800 卢恩，不恢复生命或专注。\n选择后，本处泉水的其他馈赠将不可领取。\n\n演示确认流程？本页不会改变角色或本局进度。", L"卢恩领取流程预览完成。\n\n正式效果：获得 800 卢恩。\n本次仅预览，未发放卢恩，也未消耗事件。"},
}};
const std::array<PreviewChoice,2> dice_choices{{
    {L"小试身手　·　押 1000 卢恩", L"胜：拿回 2000 卢恩，净赚 1000。负：失去赌注 1000。每处赌局只能参与一次。此页仅演示，输赢交替展示，不改变存档。",
     L"骰子赌徒 · 小试身手\n\n赌注：1000 卢恩\n获胜：拿回 2000（净赚 1000）\n失败：失去赌注（净亏 1000）\n\n演示掷骰？本页不会扣款或结算。",
     L"骰子落定 · 获胜预览\n\n赌徒将卢恩推到你面前。\n拿回 2000 卢恩，其中赌注 1000，净赚 1000。\n\n演示结果：未发放卢恩，未消耗事件。",
     L"骰子落定 · 失败预览\n\n赌徒拢起桌上的卢恩。\n损失赌注 1000 卢恩，未获返还。\n\n演示结果：未扣除卢恩，未消耗事件。"},
    {L"放手一搏　·　押 3000 卢恩", L"胜：拿回 6000 卢恩，净赚 3000。负：失去赌注 3000。每处赌局只能参与一次。此页仅演示，输赢交替展示，不改变存档。",
     L"骰子赌徒 · 放手一搏\n\n赌注：3000 卢恩\n获胜：拿回 6000（净赚 3000）\n失败：失去赌注（净亏 3000）\n\n演示掷骰？本页不会扣款或结算。",
     L"骰子落定 · 获胜预览\n\n赌徒将卢恩推到你面前。\n拿回 6000 卢恩，其中赌注 3000，净赚 3000。\n\n演示结果：未发放卢恩，未消耗事件。",
     L"骰子落定 · 失败预览\n\n赌徒拢起桌上的卢恩。\n损失赌注 3000 卢恩，未获返还。\n\n演示结果：未扣除卢恩，未消耗事件。"},
}};
std::atomic_bool preview_pending{false};
std::atomic_uint dice_preview_count{0};
bool preview_enabled() noexcept {
    wchar_t path[32768]{};
    const auto size=GetModuleFileNameW(module,path,32768);
    if(!size || size>=32768) return false;
    try {std::error_code error;return std::filesystem::exists(std::filesystem::path(path).parent_path().parent_path()/L"tower-native-ui-preview.enable",error);}
    catch(...) {return false;}
}
void preview_result_closed(void*, ERUI_Result, erui::AlertResponse) noexcept {preview_pending.store(false);}
void preview_confirmed(void* context, ERUI_Result status, erui::AlertResponse response) noexcept {
    if(status!=ERUI_OK || response!=erui::AlertResponse::primary) {preview_pending.store(false);return;}
    const auto* choice=static_cast<const PreviewChoice*>(context);
    const erui::AlertOptions options{erui::AlertButtons::ok,erui::AlertPlacement::center};
    const auto* result=choice->alternate_result && (dice_preview_count.fetch_add(1)%2) ? choice->alternate_result : choice->result;
    if(registration.alert(tr(result),options,preview_result_closed)!=ERUI_OK) preview_pending.store(false);
}
void event_preview(void* context) noexcept {
    if(!ready.load(std::memory_order_acquire) || preview_pending.exchange(true)) return;
    const auto* choice=static_cast<const PreviewChoice*>(context);
    const erui::AlertOptions options{erui::AlertButtons::yes_no,erui::AlertPlacement::center};
    if(registration.alert(tr(choice->confirm),options,preview_confirmed,context)!=ERUI_OK) preview_pending.store(false);
}
void build_event_preview(erui::Page& settings) noexcept {
    auto previews=settings.add_submenu(tr(L"事件页面预览（开发）"),tr(L"仅演示原生页面与确认流程，不发奖励、不消耗事件。"),tr(L"黄金树爬塔 · 事件预览"),tr(L"原生事件页面的开发预览，返回键随时离开。"));
    auto spring=previews.add_submenu(tr(L"月光泉　·　栖息片刻"),tr(L"银白的泉水映着碎月。恢复生命、恢复专注、领取 800 卢恩，三者择一。"),tr(L"奇遇 · 月光泉（预览）"),tr(L"泉水无声流淌。你可以留步休憩，也可以带走泉畔的财富。"));
    for(const auto& choice:spring_choices) spring.add_button(tr(choice.label),tr(choice.help),event_preview,const_cast<PreviewChoice*>(&choice));
    auto dice=previews.add_submenu(tr(L"骰子赌徒　·　孤注一掷"),tr(L"两枚骨骰在掌心作响。押上卢恩，赢则翻倍拿回，输则失去赌注。"),tr(L"奇遇 · 骰子赌徒（预览）"),tr(L"“押上卢恩，赢了翻倍，输了归我。”选择赌注，确认后查看演示结果。"));
    for(const auto& choice:dice_choices) dice.add_button(tr(choice.label),tr(choice.help),event_preview,const_cast<PreviewChoice*>(&choice));
}

void changed(void* context, std::uint8_t value) noexcept {
    *static_cast<std::uint8_t*>(context) = value;
}
void action_pressed(unsigned& slot, const erui::ActionActivation&) noexcept {
    if (ready.load(std::memory_order_acquire) && activate) activate(slot);
}
void assignments_changed(const erui::AssignmentsChangedEvent& event) noexcept {
    if (!ready.load(std::memory_order_acquire)) return;
    auto next = key_codes;
    for (const auto change : event.changes()) {
        for (unsigned i=0;i<5;++i) if (change.action_id()==action_ids[i]) {
            const auto key=change.current().keyboard_input();
            next[i]=key ? static_cast<unsigned>(*key) : 0;
        }
    }
    if (save_bindings && save_bindings(next.data())) {key_codes=next;return;}
    for (const auto change : event.changes()) (void)change.action().bind(change.previous());
    (void)registration.alert(tr(L"按键保存失败，已恢复原绑定。请查看 tower-preview.log。"));
}
void binding_help() noexcept {
    if (ready.load(std::memory_order_acquire)) (void)registration.alert(tr(L"请进入系统 → 键盘／鼠标配置 → 按键配置，找到黄金树爬塔分组。选中动作后按下新键，修改和清除绑定会自动保存并立即生效。"));
}
void apply() noexcept {
    if (!ready.load(std::memory_order_acquire)) return;
    const bool ok = save && save(values.data()) != 0;
    (void)registration.alert(ok ? tr(L"设置已保存。请保存退出并重新启动游戏后生效。") : tr(L"设置保存失败，请查看 tower-preview.log。原配置未替换。"));
}
}

extern "C" __declspec(dllexport) unsigned __cdecl tower_native_settings_init_v3(
    const std::uint8_t* initial, const unsigned* keys, const unsigned* defaults, Save callback, SaveBindings bindings_callback, Activate activation_callback) noexcept {
    try {
        if (!initial || !keys || !defaults || !callback || !bindings_callback || !activation_callback) return ERUI_INVALID_ARGUMENT;
        if (ready.load()) return ERUI_ALREADY_COMMITTED;
        std::copy_n(initial, values.size(), values.begin());
        save = callback;
        save_bindings = bindings_callback;
        activate = activation_callback;
        std::copy_n(keys,5,key_codes.begin());
        auto connection = erui::connect();
        for (unsigned i=0; !connection && i<30; ++i) { Sleep(50); connection = erui::connect(); }
        if (!connection) return 1000 + connection.error().native_result();
        auto game_language=connection.value().game_language();
        for(unsigned i=0;language_mode==0&&!game_language.available()&&i<30;++i) {Sleep(50);game_language=connection.value().game_language();}
        english.store(language_mode==2 || (language_mode==0 && game_language.known==erui::GameLanguage::english));
        erui::ProviderOptions provider{};
        provider.provider_id = "erdtree-tower";
        provider.display_name = tr(L"黄金树爬塔");
        provider.owner_module = module;
        const auto result = connection.value().register_menu(provider, [&](erui::Menu& menu) {
            auto page = menu.root().add_submenu(tr(L"黄金树爬塔"), tr(L"配置爬塔界面、快捷键及 DLC 内容。"), tr(L"黄金树爬塔"), tr(L"修改后选择保存设置，重新启动游戏后生效。"));
            if(preview_enabled()) build_event_preview(page);
            page.add_button<&apply>(tr(L"保存设置（重启生效）"), tr(L"修改后选择此项保存。设置与启动器共用；保存退出并重新启动游戏后生效。"));
            page.add_toggle(tr(L"调试菜单"), tr(L"启用爬塔调试入口。独立的 tower-debug.enable 文件仍可强制启用。"), values[0], changed, &values[0]);
            erui::SliderOptions volume{};
            volume.initial_value = values[1];
            page.add_slider(tr(L"爬塔界面音量"), tr(L"调整爬塔界面音效，0 为关闭；不修改游戏音乐和战斗音量。"), volume, changed, &values[1]);
            const std::wstring_view modes[]{tr(L"完整 DLC"), tr(L"仅本体"), tr(L"本体＋DLC 武器（实验）")};
            page.add_popup_choice(tr(L"DLC 内容"), tr(L"联机时队伍里有人没有 DLC，整队都按没有 DLC 运行。"), {modes, 3, values[2]}, changed, &values[2]);
            const wchar_t* labels[]{tr(L"快速结算快捷键"), tr(L"显示／隐藏界面快捷键"), tr(L"旅程档案快捷键"), tr(L"构筑面板快捷键"), tr(L"调试菜单快捷键")};
            page.add_button<&binding_help>(tr(L"快捷键配置位置"), tr(L"快捷键已移至原生按键配置中的黄金树爬塔分组，修改后自动保存并生效。"));
            auto bindings=menu.input_bindings();
            auto section=bindings.add_section(tr(L"黄金树爬塔"));
            for (unsigned i=0; i<5; ++i) {
                auto action=section.add_action<&action_pressed>(action_ids[i],labels[i],erui::inputs::keyboard(static_cast<erui::KeyboardKey>(defaults[i])),slots[i]);
                auto input=erui::inputs::keyboard();
                if(keys[i]) input.keyboard(static_cast<erui::KeyboardKey>(keys[i]));
                if (action.bind(input)!=ERUI_OK) throw 1;
            }
            if(bindings.on_assignments_changed<&assignments_changed>()!=ERUI_OK) throw 1;
        });
        if (!result) return 2000 + result.error().native_result();
        registration = result.value();
        ready.store(true, std::memory_order_release);
        return ERUI_OK;
    } catch (...) { return ERUI_INTERNAL_ERROR; }
}

extern "C" __declspec(dllexport) void __cdecl tower_native_settings_set_language(unsigned mode) noexcept {language_mode=mode;}
extern "C" __declspec(dllexport) unsigned __cdecl tower_native_settings_language() noexcept {return english.load()?1u:0u;}

BOOL APIENTRY DllMain(HMODULE instance, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) { module = instance; DisableThreadLibraryCalls(instance); }
    return TRUE;
}
