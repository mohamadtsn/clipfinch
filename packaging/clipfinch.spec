Name:           clipfinch
Version:        %{clipfinch_version}
Release:        1%{?dist}
Summary:        Clipboard history for Linux
License:        MIT

%description
Clipboard history with search, pinboards and a paste queue for GTK desktops.

%install
install -Dm755 %{projectdir}/target/release/clipfinch %{buildroot}%{_bindir}/clipfinch
install -Dm644 %{projectdir}/assets/clipfinch.desktop %{buildroot}%{_datadir}/applications/clipfinch.desktop
install -Dm644 %{projectdir}/assets/icon.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/clipfinch.png
install -Dm644 %{projectdir}/LICENSE %{buildroot}%{_licensedir}/clipfinch/LICENSE

%files
%{_bindir}/clipfinch
%{_datadir}/applications/clipfinch.desktop
%{_datadir}/icons/hicolor/512x512/apps/clipfinch.png
%license %{_licensedir}/clipfinch/LICENSE
