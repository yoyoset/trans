import { Component, type ErrorInfo, type ReactNode } from "react";

import { Button } from "./Button";

interface State {
  error: Error | null;
}

/**
 * 某个面板渲染崩溃时显示错误和"重试"，而不是整个窗口黑屏。
 * 首次拉起时就踩过：一个 effect 返回了 Promise，React 抛错后整棵树卸载，窗口一片黑。
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI crashed", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="space-y-3 rounded-lg border border-rose-900 bg-rose-950/50 p-4 text-rose-200">
        <p className="font-medium">界面出错了，其他功能不受影响。</p>
        <details className="text-xs text-rose-300/80">
          <summary className="cursor-pointer select-none">技术详情</summary>
          <pre className="mt-1 whitespace-pre-wrap break-all font-mono">{error.stack ?? error.message}</pre>
        </details>
        <Button onClick={() => this.setState({ error: null })}>重试</Button>
      </div>
    );
  }
}
