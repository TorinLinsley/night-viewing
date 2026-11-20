@echo off
chcp 65001 >nul
cd /d "%~dp0"

git add -A

git diff --cached --quiet
if %errorlevel%==0 goto nochanges

git commit -m "update: %date% %time%"
if errorlevel 1 goto fail

git push
if errorlevel 1 goto fail

echo.
echo 推送成功
pause
exit /b 0

:nochanges
echo.
echo 没有检测到更改
pause
exit /b 0

:fail
echo.
echo 操作失败，请检查上面的错误信息
pause
exit /b 1