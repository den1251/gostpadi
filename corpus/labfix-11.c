/* ЛБ3, задача 3: сортировка массива.
 * Пузырёк по возрастанию, вывод до и после. */
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define N_MAX 100

int main(void) {
    int a[N_MAX];
    int n = 0;
    int i = 0;
    int j = 0;
    int tmp = 0;

    while (1) {
        printf("n (1..%d): ", N_MAX);
        if (scanf("%d", &n) == 1 && n >= 1 && n <= N_MAX) {
            break;
        }
        printf("нужно 1..%d, повтори\n", N_MAX);
        while (getchar() != '\n') {
        }
    }
    srand(time(NULL));
    for (i = 0; i < n; i++) {
        a[i] = rand() % 200 - 100;
    }
    printf("было:\n");
    for (i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    for (i = 0; i < n - 1; i++) {
        for (j = 0; j < n - 1 - i; j++) {
            if (a[j] > a[j + 1]) {
                tmp = a[j];
                a[j] = a[j + 1];
                a[j + 1] = tmp;
            }
        }
    }
    printf("стало:\n");
    for (i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    return 0;
}
