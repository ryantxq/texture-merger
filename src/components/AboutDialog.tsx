// src/components/AboutDialog.tsx
export default function AboutDialog({ onClose }: { onClose: () => void }) {
  return (
    <div className="modal-mask" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          关于 贴图合并工具
          <button className="btn icon" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div><b>版本</b>　v1.0.0</div>
          <div><b>作者</b>　@月石MoonStone</div>
          <div><b>联系</b>　ryantxq@sina.com</div>
          <div style={{ marginTop: 10, borderTop: "1px dashed var(--border-2)", paddingTop: 8 }}>
            <b>使用说明</b>
            <ol style={{ margin: "4px 0 0", paddingLeft: 18 }}>
              <li>将 3ds Max 导出的同尺寸带透明通道 PNG 拖入窗口，或通过「导入 / 选择文件夹」批量添加。</li>
              <li>在左侧图层列表调整顺序（顶部=最上层）、旋转/翻转，可用眼睛图标临时隐藏图层。</li>
              <li>在右侧画布实时预览重叠合成效果，双击图层可单独查看该层像素。</li>
              <li>设置位深与压缩级别，点击「导出 PNG」选择保存位置完成导出。</li>
            </ol>
          </div>
        </div>
        <div className="modal-foot">
          <button className="btn primary" onClick={onClose}>关闭</button>
        </div>
      </div>
    </div>
  );
}
