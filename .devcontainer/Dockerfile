FROM mcr.microsoft.com/devcontainers/base:ubuntu

RUN sudo apt update
RUN sudo apt full-upgrade --yes

# Enable the add-apt-repository command
RUN sudo apt install --yes software-properties-common

# Add the main repository for mold
RUN sudo add-apt-repository main --yes

# Install mold
RUN sudo apt install --yes mold pkg-config