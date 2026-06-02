import SwiftUI

struct GeneralTab: View {
    @ObservedObject var viewModel: SettingsViewModel

    var body: some View {
        Form {
            Section("外观") {
                Picker("主题", selection: $viewModel.config.general.theme) {
                    Text("跟随系统").tag(ThemeMode.system)
                    Text("浅色").tag(ThemeMode.light)
                    Text("深色").tag(ThemeMode.dark)
                }
                .pickerStyle(.segmented)
            }

            Section("Markdown 渲染") {
                Picker("渲染方式", selection: $viewModel.config.general.markdownRenderer) {
                    Text("原生（分块，加载快）").tag(MarkdownRenderMode.native)
                    Text("WebView（可整段选中）").tag(MarkdownRenderMode.webview)
                }
                .pickerStyle(.inline)
            }

            Section("弹窗行为") {
                Toggle("窗口置顶", isOn: $viewModel.config.general.alwaysOnTop)
            }
        }
        .formStyle(.grouped)
        .padding()
    }
}
