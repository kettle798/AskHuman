import AppKit

@MainActor
final class PopupChannel: InteractionChannel {
    let id = "popup"

    private let config: AppConfig
    private var controller: PopupWindowController?

    init(config: AppConfig) {
        self.config = config
    }

    func start(request: AskRequest, completion: @escaping (ChannelResult) -> Void) {
        let viewModel = PopupViewModel(
            request: request,
            markdownMode: config.general.markdownRenderer,
            theme: config.general.theme,
            onResult: completion
        )
        let controller = PopupWindowController(viewModel: viewModel, config: config)
        self.controller = controller
        controller.show()
    }

    func cancelByOtherChannel() {
        controller?.closeSilently()
        controller = nil
    }
}
