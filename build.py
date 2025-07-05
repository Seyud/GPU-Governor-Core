#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
GPU Governor 编译脚本
自动执行Rust项目的编译工作，默认不进行UPX压缩
使用 --with-upx 参数可以同时进行编译和压缩
"""

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


# 在导入任何其他模块之前立即清理PATH环境变量中的双引号字符
# 这样可以防止Python扩展加载时出现问题
def _clean_path_environment():
    """清理PATH环境变量中的双引号字符，防止Python扩展加载失败"""
    path_var = os.environ.get("PATH", "")
    if '"' in path_var:
        clean_path = path_var.replace('"', "")
        os.environ["PATH"] = clean_path
        print("已清理PATH环境变量中的双引号字符")

    # 同时清理其他可能包含路径的环境变量
    for env_var in ["Path", "path"]:
        if env_var in os.environ:
            value = os.environ[env_var]
            if '"' in value:
                clean_value = value.replace('"', "")
                os.environ[env_var] = clean_value
                print(f"已清理{env_var}环境变量中的双引号字符")


# 立即执行清理操作
_clean_path_environment()


def clean_path_string(path_str):
    """
    清理路径字符串中的双引号字符
    Args:
        path_str: 路径字符串
    Returns:
        清理后的路径字符串
    """
    if isinstance(path_str, str):
        return path_str.replace('"', "").replace("'", "")
    return str(path_str).replace('"', "").replace("'", "")


class GPUGovernorBuilder:
    def __init__(self):
        # 清理系统PATH中的双引号字符，防止Python扩展加载失败
        self._clean_system_path()

        # 配置路径（确保没有双引号字符）
        self.android_ndk_home = clean_path_string("D:/android-ndk-r27c")
        self.llvm_path = clean_path_string("D:/LLVM")
        self.upx_path = clean_path_string("D:/upx/upx.exe")

        # 项目配置
        self.target = "aarch64-linux-android"
        self.binary_name = "gpugovernor"
        self.output_dir = "output"

        # 设置环境变量
        self._setup_environment()

    def _clean_system_path(self):
        """清理系统PATH环境变量中的双引号字符（额外保险措施）"""
        # 清理所有可能的PATH相关环境变量
        path_vars = ["PATH", "Path", "path"]

        for var_name in path_vars:
            if var_name in os.environ:
                current_path = os.environ[var_name]
                if '"' in current_path:
                    clean_path = current_path.replace('"', "")
                    os.environ[var_name] = clean_path
                    print(f"已清理{var_name}环境变量中的双引号字符")

        # 同时清理其他可能包含路径且有双引号的环境变量
        for key, value in list(os.environ.items()):
            if (
                isinstance(value, str)
                and '"' in value
                and any(keyword in key.lower() for keyword in ["path", "dir", "home"])
            ):
                clean_value = value.replace('"', "")
                os.environ[key] = clean_value
                print(f"已清理{key}环境变量中的双引号字符")

    def _setup_environment(self):
        """设置编译所需的环境变量"""
        env_vars = {
            "ANDROID_NDK_HOME": self.android_ndk_home,
            "LLVM_PATH": self.llvm_path,
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER": f"{self.android_ndk_home}/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android33-clang.cmd",
            "LIBCLANG_PATH": f"{self.llvm_path}/bin",
            "BINDGEN_EXTRA_CLANG_ARGS": f"--target=aarch64-linux-android -I{self.android_ndk_home}/toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/include",
        }

        # 更新PATH环境变量，清理双引号字符
        current_path = os.environ.get("PATH", "")
        # 移除PATH中的双引号字符，防止Python扩展加载失败
        current_path = clean_path_string(current_path)
        new_path_parts = [
            f"{self.llvm_path}/bin",
            f"{self.android_ndk_home}/toolchains/llvm/prebuilt/windows-x86_64/bin",
            current_path,
        ]
        env_vars["PATH"] = clean_path_string(";".join(new_path_parts))

        # 设置环境变量，清理所有值中的双引号字符
        for key, value in env_vars.items():
            # 移除环境变量值中的双引号字符，防止Python扩展加载失败
            clean_value = clean_path_string(str(value))
            os.environ[key] = clean_value
            print(f"设置环境变量: {key}={clean_value}")

    def _check_dependencies(self):
        """检查编译依赖是否存在"""
        dependencies = [
            (self.android_ndk_home, "Android NDK"),
            (self.llvm_path, "LLVM"),
            (self.upx_path, "UPX"),
        ]

        missing_deps = []
        for path, name in dependencies:
            if not os.path.exists(path):
                missing_deps.append(f"{name}: {path}")

        if missing_deps:
            print("错误：以下依赖项未找到：")
            for dep in missing_deps:
                print(f"  - {dep}")
            return False

        print("所有依赖项检查通过")
        return True

    def build(self):
        """执行Rust项目编译"""
        print("开始编译Rust项目...")

        # 检查依赖
        if not self._check_dependencies():
            return False

        # 执行cargo fmt --check命令
        print("检查代码格式...")
        fmt_cmd = ["cargo", "fmt", "--check"]
        print(f"执行命令: {' '.join(fmt_cmd)}")

        try:
            result = subprocess.run(
                fmt_cmd,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="ignore",
            )
            print("代码格式检查通过！")
            if result.stdout:
                print(result.stdout)
        except subprocess.CalledProcessError as e:
            print(f"代码格式检查失败：{e}")
            if hasattr(e, "stderr") and e.stderr:
                print(f"错误输出：{e.stderr}")

            # 自动修复代码格式
            print("正在自动修复代码格式...")
            fmt_fix_cmd = ["cargo", "fmt"]
            print(f"执行命令: {' '.join(fmt_fix_cmd)}")

            try:
                fix_result = subprocess.run(
                    fmt_fix_cmd,
                    check=True,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="ignore",
                )
                print("代码格式修复成功！")
                if fix_result.stdout:
                    print(fix_result.stdout)

                # 再次检查格式是否正确
                print("再次检查代码格式...")
                recheck_result = subprocess.run(
                    fmt_cmd,
                    check=True,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="ignore",
                )
                print("代码格式检查通过！")

            except subprocess.CalledProcessError as fix_e:
                print(f"代码格式修复失败：{fix_e}")
                if hasattr(fix_e, "stderr") and fix_e.stderr:
                    print(f"错误输出：{fix_e.stderr}")
                print("请手动运行 'cargo fmt' 来格式化代码")
                return False

        # 执行cargo clippy代码检查
        print("执行代码质量检查...")
        clippy_cmd = ["cargo", "clippy", "--", "-D", "warnings"]
        print(f"执行命令: {' '.join(clippy_cmd)}")

        try:
            result = subprocess.run(
                clippy_cmd,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="ignore",
            )
            print("代码质量检查通过！")
            if result.stdout:
                print(result.stdout)
        except subprocess.CalledProcessError as e:
            print(f"代码质量检查失败：{e}")
            if hasattr(e, "stderr") and e.stderr:
                print(f"错误输出：{e.stderr}")
            if hasattr(e, "stdout") and e.stdout:
                print(f"标准输出：{e.stdout}")
            print("请修复上述警告和错误后重新编译")
            return False

        # 执行cargo build命令
        cmd = ["cargo", "build", "--release", "--target", self.target]
        print(f"执行命令: {' '.join(cmd)}")

        try:
            result = subprocess.run(
                cmd,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="ignore",
            )
            print("编译成功！")
            if result.stdout:
                print(result.stdout)
            return True
        except subprocess.CalledProcessError as e:
            print(f"编译失败：{e}")
            if hasattr(e, "stderr") and e.stderr:
                print(f"错误输出：{e.stderr}")
            return False

    def copy_binary(self):
        """复制编译后的二进制文件到输出目录"""
        source_path = f"target/{self.target}/release/{self.binary_name}"

        if not os.path.exists(source_path):
            print(f"错误：编译输出文件未找到：{source_path}")
            return False

        # 创建输出目录
        os.makedirs(self.output_dir, exist_ok=True)

        # 复制文件
        dest_path = f"{self.output_dir}/{self.binary_name}"
        shutil.copy2(source_path, dest_path)

        # 显示文件大小
        file_size = os.path.getsize(dest_path)
        print(f"二进制文件已复制到：{dest_path}")
        print(f"文件大小：{file_size:,} 字节")

        return True

    def compress(self):
        """使用UPX压缩二进制文件"""
        binary_path = f"{self.output_dir}/{self.binary_name}"

        if not os.path.exists(binary_path):
            print(f"错误：二进制文件未找到：{binary_path}")
            return False

        if not os.path.exists(self.upx_path):
            print(f"错误：UPX工具未找到：{self.upx_path}")
            return False

        # 获取压缩前文件大小
        original_size = os.path.getsize(binary_path)
        print(f"压缩前文件大小：{original_size:,} 字节")

        # 先创建压缩版本的副本（在压缩之前）
        compressed_copy = f"{self.output_dir}/{self.binary_name}_compressed"
        shutil.copy2(binary_path, compressed_copy)
        print(f"已创建待压缩文件副本：{compressed_copy}")

        # 对副本执行UPX压缩，保持原文件不变
        cmd = [self.upx_path, "--lzma", compressed_copy]
        print(f"执行UPX压缩：{' '.join(cmd)}")

        try:
            result = subprocess.run(
                cmd,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="ignore",
            )
            print("UPX压缩成功！")

            # 获取压缩后文件大小
            compressed_size = os.path.getsize(compressed_copy)
            ratio = (compressed_size / original_size) * 100

            print(f"压缩后文件大小：{compressed_size:,} 字节 ({ratio:.2f}% 的原始大小)")
            print(f"原始文件保持不变：{binary_path}")
            print(f"压缩后的文件：{compressed_copy}")

            return True

        except subprocess.CalledProcessError as e:
            print(f"UPX压缩失败：{e}")
            if hasattr(e, "stderr") and e.stderr:
                print(f"错误输出：{e.stderr}")
            # 如果压缩失败，删除创建的副本
            if os.path.exists(compressed_copy):
                os.remove(compressed_copy)
            return False

    def clean(self):
        """清理编译输出"""
        print("清理编译输出...")

        # 清理cargo输出
        try:
            subprocess.run(
                ["cargo", "clean"], check=True, encoding="utf-8", errors="ignore"
            )
            print("Cargo清理完成")
        except subprocess.CalledProcessError as e:
            print(f"Cargo清理失败：{e}")

        # 清理输出目录
        if os.path.exists(self.output_dir):
            shutil.rmtree(self.output_dir)
            print(f"输出目录已清理：{self.output_dir}")

    def build_only_flow(self):
        """执行编译流程（默认行为）"""
        print("=" * 50)
        print("GPU Governor 编译脚本")
        print("=" * 50)

        # 编译
        if not self.build():
            print("编译失败，停止执行")
            return False

        # 复制二进制文件
        if not self.copy_binary():
            print("复制二进制文件失败，停止执行")
            return False

        print("=" * 50)
        print("编译完成！")
        print("=" * 50)
        return True

    def build_and_compress(self):
        """执行完整的编译和压缩流程"""
        print("=" * 50)
        print("GPU Governor 编译和压缩脚本")
        print("=" * 50)

        # 编译
        if not self.build():
            print("编译失败，停止执行")
            return False

        # 复制二进制文件
        if not self.copy_binary():
            print("复制二进制文件失败，停止执行")
            return False

        # 压缩
        if not self.compress():
            print("压缩失败，但编译成功")
            return True

        print("=" * 50)
        print("编译和压缩完成！")
        print("=" * 50)
        return True


def main():
    parser = argparse.ArgumentParser(description="GPU Governor 编译脚本")
    parser.add_argument("--clean", action="store_true", help="清理编译输出")
    parser.add_argument("--with-upx", action="store_true", help="编译并使用UPX压缩")
    parser.add_argument(
        "--compress-only", action="store_true", help="仅压缩现有二进制文件"
    )

    args = parser.parse_args()

    builder = GPUGovernorBuilder()

    if args.clean:
        builder.clean()
        return

    if args.compress_only:
        if builder.compress():
            print("压缩完成")
        else:
            print("压缩失败")
            sys.exit(1)
        return

    if args.with_upx:
        # 编译和压缩流程
        if not builder.build_and_compress():
            sys.exit(1)
        return

    # 默认：仅编译流程
    if not builder.build_only_flow():
        sys.exit(1)


if __name__ == "__main__":
    main()
